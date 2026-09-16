//! Body input and semantic selection actions.
use super::*;

impl SpikeApp {
    /// 当前焦点对应的可编辑文本位置。
    pub(super) fn target(&self) -> Option<(NodeId, usize)> {
        match self.focus {
            Cursor::Text { node, byte } => Some((node, byte)),
            Cursor::Slot { node, slot, index } => {
                let document = self.core.document();
                let list = document.slot(node, slot).ok()?;
                let child = list.get(index).or_else(|| list.last())?;
                document
                    .first_text_descendant(*child)
                    .map(|text_node| (text_node, 0))
            }
        }
    }

    /// 把焦点节点包进一个结构。
    ///
    /// 与 Iced 候选的同名能力对齐：结构编辑是验收项之一，候选必须能真的执行，
    /// 而不只是核心里有这两个 API。
    pub fn wrap(&mut self, kind: NodeKind) {
        let node = self.focus.focus();
        let edit = SemanticEdit::Wrap { node, kind };
        match self.core.apply(LOCAL, Intent::Structural, edit) {
            Ok(Some(_)) => {
                // 焦点跟随新结构：包裹后用户接下来的操作（解除、循环变体）显然应作用于它。
                // Wrap 把新结构放在被包裹节点的原位置，所以从被包裹节点的父节点就能取到。
                if let Some((wrapper, _, _)) = self.core.document().locate_in_parent(node) {
                    self.focus = Cursor::Slot {
                        node: wrapper,
                        slot: 0,
                        index: 0,
                    };
                    self.selection = Selection::collapsed(self.focus);
                }
                self.last_event = format!("core：包裹为 {kind:?}");
            }
            Ok(None) => self.last_event = "包裹：无变化".to_string(),
            Err(error) => self.last_event = format!("包裹被拒：{error}"),
        }
    }

    /// 解除最内层结构，保留内容。
    pub fn unwrap(&mut self) {
        let node = self.focus.focus();
        let target = self.target();
        let edit = SemanticEdit::Unwrap { node };
        match self.core.apply(LOCAL, Intent::Structural, edit) {
            Ok(Some(_)) => {
                if let Some((node, byte)) = target {
                    self.focus = Cursor::Text { node, byte };
                    self.selection = Selection::collapsed(self.focus);
                }
                self.last_event = "core：解除结构".to_string();
            }
            Ok(None) => self.last_event = "解除：无变化".to_string(),
            Err(error) => self.last_event = format!("解除被拒：{error}"),
        }
    }

    /// 循环结构变体（例如分数 ↔ 根式）。
    pub fn cycle_variant(&mut self) {
        let node = self.focus.focus();
        let edit = SemanticEdit::CycleVariant { node };
        match self.core.apply(LOCAL, Intent::Structural, edit) {
            Ok(Some(_)) => self.last_event = "core：循环结构变体".to_string(),
            Ok(None) => self.last_event = "循环变体：无变化".to_string(),
            Err(error) => self.last_event = format!("循环变体被拒：{error}"),
        }
    }

    pub(super) fn insert_text(&mut self, text: &str, intent: Intent) {
        let (node, at, mut edits) = if self.selection.is_collapsed() {
            let Some((node, at)) = self.target() else {
                self.last_event = "没有可编辑的文本槽位".to_string();
                return;
            };
            (node, at, Vec::new())
        } else {
            match scholium_spike_core::selection_edit::deletion(
                self.core.document(),
                self.selection,
            ) {
                Ok((Cursor::Text { node, byte }, edits)) => (node, byte, edits),
                Ok(_) => return,
                Err(error) => {
                    self.last_event = error.to_string();
                    return;
                }
            }
        };
        edits.push(SemanticEdit::InsertText {
            node,
            at,
            text: text.to_string(),
        });
        match self.core.apply_batch(LOCAL, intent, &edits) {
            Ok(Some(_)) => {
                self.focus = Cursor::Text {
                    node,
                    byte: at + text.len(),
                };
                self.selection = Selection::collapsed(self.focus);
                self.last_event = format!("core 收到 {intent:?}：\"{text}\"");
            }
            Ok(None) => self.last_event = "空操作".to_string(),
            Err(error) => self.last_event = format!("错误：{error}"),
        }
    }

    pub(super) fn navigate(&mut self, direction: Direction) {
        match move_cursor(self.core.document(), self.focus, direction) {
            Ok(Some(next)) => {
                self.focus = next;
                self.last_event = format!("焦点 → {:?}", next.focus().index());
            }
            Ok(None) => self.last_event = format!("{direction:?}：没有可取位置"),
            Err(error) => self.last_event = format!("错误：{error}"),
        }
    }

    /// 处理键盘与输入法事件。
    ///
    /// 输入法事件是全局的，因此这里统一按"当前焦点在正文区"处理；源码面板只读时不参与。
    pub(super) fn handle_input(&mut self, ctx: &egui::Context) {
        // 输入焦点不在正文区时，输入法与按键都不属于它；否则会与源码编辑器抢输入
        // （这正是 Iced 候选当初踩过的坑）。
        if !self.structure_focused
            || !ctx.input(|input| input.focused)
            || self
                .structure_id
                .is_some_and(|id| !ctx.memory(|memory| memory.has_focus(id)))
        {
            self.interrupt_ime |= !self.preedit.is_empty();
            self.preedit.clear();
            return;
        }

        self.text_events(ctx);
        if self.preedit.is_empty() {
            self.navigation_keys(ctx);
        }
    }

    fn text_events(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|input| input.events.clone());
        for event in events {
            match event {
                egui::Event::Copy | egui::Event::Cut => {
                    if self.selection.is_collapsed() {
                        continue;
                    }
                    match scholium_spike_core::selection_edit::plain_text(
                        self.core.document(),
                        self.selection,
                    ) {
                        Ok(text) => {
                            ctx.copy_text(text);
                            if matches!(event, egui::Event::Cut) {
                                self.delete_backward();
                            }
                        }
                        Err(error) => self.last_event = error.to_string(),
                    }
                }
                egui::Event::Paste(text) => self.insert_text(&text, Intent::Paste),
                egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                    self.preedit_events += 1;
                    self.preedit = text;
                    self.last_event = format!("预编辑「{}」（未进历史）", self.preedit);
                }
                egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                    self.preedit.clear();
                    if text.is_empty() {
                        self.last_event = "IME 提交空串，忽略".to_string();
                    } else {
                        self.commits += 1;
                        self.insert_text(&text, Intent::ImeCommit);
                    }
                }
                egui::Event::Text(text) if self.preedit.is_empty() && !text.is_empty() => {
                    self.insert_text(&text, Intent::Typing);
                }
                _ => {}
            }
        }
    }

    fn navigation_keys(&mut self, ctx: &egui::Context) {
        let (down, up, left, right, backspace, undo, extend) = ctx.input(|input| {
            (
                input.key_pressed(egui::Key::ArrowDown),
                input.key_pressed(egui::Key::ArrowUp),
                input.key_pressed(egui::Key::ArrowLeft),
                input.key_pressed(egui::Key::ArrowRight),
                input.key_pressed(egui::Key::Backspace),
                input.modifiers.command && input.key_pressed(egui::Key::Z),
                input.modifiers.shift,
            )
        });

        let mut moved = false;
        if down {
            self.navigate(Direction::Down);
            moved = true;
        }
        if up {
            self.navigate(Direction::Up);
            moved = true;
        }
        if left {
            self.navigate(Direction::Parent);
            moved = true;
        }
        if right {
            self.navigate(Direction::FirstChild);
            moved = true;
        }
        if moved {
            self.sync_selection(extend);
        }
        if backspace {
            self.delete_backward();
        }
        if undo {
            self.undo();
        }
    }

    /// 光标移动后同步选区：按住 Shift 时保留锚点（扩展），否则折叠到光标处。
    pub(super) fn sync_selection(&mut self, extend: bool) {
        if extend {
            self.selection.focus = self.focus;
        } else {
            self.selection = Selection::collapsed(self.focus);
        }
    }

    pub(super) fn delete_backward(&mut self) {
        // 跨文本叶子的选区作为一个原子动作删除，完整覆盖的中间结构一起移除。
        if !self.selection.is_collapsed() {
            match scholium_spike_core::selection_edit::deletion(
                self.core.document(),
                self.selection,
            ) {
                Ok((caret, edits)) => {
                    match self.core.apply_batch(LOCAL, Intent::Structural, &edits) {
                        Ok(_) => {
                            self.focus = caret;
                            self.selection = Selection::collapsed(caret);
                            self.last_event = "已删除选区".to_owned();
                        }
                        Err(error) => self.last_event = error.to_string(),
                    }
                }
                Err(error) => self.last_event = error.to_string(),
            }
            return;
        }
        let Some((node, at)) = self.target() else {
            return;
        };
        let edit = SemanticEdit::DeleteBackward { node, at };
        let previous = self
            .core
            .document()
            .node(node)
            .ok()
            .and_then(|n| n.text.prev_grapheme_boundary(at))
            .unwrap_or(0);
        match self.core.apply(LOCAL, Intent::Typing, edit) {
            Ok(Some(_)) => {
                self.focus = Cursor::Text {
                    node,
                    byte: previous,
                };
                self.selection = Selection::collapsed(self.focus);
                self.last_event = "core：删除一个字素".to_string();
            }
            Ok(None) => self.last_event = "退格：已在开头".to_string(),
            Err(error) => self.last_event = format!("错误：{error}"),
        }
    }

    pub(super) fn undo(&mut self) {
        match self.core.undo(LOCAL) {
            Ok(Some(outcome)) => {
                self.repair_focus();
                self.last_event = format!(
                    "core：撤销 {:?}，删除 {} 字符",
                    outcome.undone, outcome.removed_chars
                );
            }
            Ok(None) => self.last_event = "撤销：本地没有可撤销动作".to_string(),
            Err(error) => self.last_event = format!("错误：{error}"),
        }
    }

    fn repair_focus(&mut self) {
        let doc = self.core.document();
        let mut node = self.focus.focus();
        let attached = node == doc.root() || {
            let mut current = node;
            while let Some((parent, _, _)) = doc.locate_in_parent(current) {
                current = parent;
            }
            current == doc.root()
        };
        if !attached {
            node = doc.root();
        }
        if let Some(leaf) = doc.first_text_descendant(node) {
            let byte = if leaf == self.focus.focus() {
                self.focus.byte().unwrap_or(0)
            } else {
                0
            };
            if let Ok(text) = doc.node(leaf) {
                let byte = text
                    .text
                    .grapheme_boundaries()
                    .into_iter()
                    .rfind(|b| *b <= byte)
                    .unwrap_or(0);
                self.focus = Cursor::Text { node: leaf, byte };
                self.selection = Selection::collapsed(self.focus);
            }
        }
    }
}

impl SpikeApp {
    pub(super) fn paint_selection(&self, painter: &egui::Painter, origin: egui::Pos2) {
        if self.selection.is_collapsed() {
            return;
        }
        let Ok((_, edits)) =
            scholium_spike_core::selection_edit::deletion(self.core.document(), self.selection)
        else {
            return;
        };
        for edit in edits {
            let SemanticEdit::DeleteRange { node, start, end } = edit else {
                continue;
            };
            let (Some(from), Some(to)) =
                (self.layout.caret(node, start), self.layout.caret(node, end))
            else {
                continue;
            };
            if (from.baseline - to.baseline).abs() > 2.0 {
                continue;
            }
            painter.rect_filled(
                egui::Rect::from_min_size(
                    origin + egui::vec2(from.x.min(to.x), Item::top_of(from.baseline, from.size)),
                    egui::vec2((to.x - from.x).abs(), from.size * 1.2),
                ),
                0.0,
                egui::Color32::from_rgb(58, 86, 132),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clipboard_cut_uses_selection_and_empty_cut_is_noop() {
        let ctx = egui::Context::default();
        let mut app = SpikeApp::new(&ctx, None);
        app.core = Editor::new();
        let node = app
            .core
            .document()
            .first_text_descendant(app.core.document().root())
            .expect("leaf");
        app.core
            .apply(
                LOCAL,
                Intent::Typing,
                SemanticEdit::InsertText {
                    node,
                    at: 0,
                    text: "abcd".into(),
                },
            )
            .expect("text");
        app.focus = Cursor::Text { node, byte: 3 };
        app.selection = Selection {
            anchor: Cursor::Text { node, byte: 1 },
            focus: app.focus,
        };
        app.structure_focused = true;
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Cut],
                ..Default::default()
            },
            |_ui| app.handle_input(&ctx),
        );
        assert!(
            output.platform_output.commands.iter().any(
                |command| matches!(command, egui::OutputCommand::CopyText(text) if text == "bc")
            )
        );
        output.textures_delta.clear();
        assert_eq!(app.plain_text(), "ad\n");
        let count = app.core.history().len();
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Cut],
                ..Default::default()
            },
            |_ui| app.handle_input(&ctx),
        );
        output.textures_delta.clear();
        assert_eq!(app.core.history().len(), count);
    }
}
