//! Evaluation-only JSON-lines process for the upstream SessionConfig fields absent from the CLI.
//! Built in an external workspace against the unmodified tagged source; never linked into Scholium.
use rtex_core::{Edit, Session, SessionConfig};
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn emit(out: &Mutex<std::io::Stdout>, value: &Value) {
    let mut out = out.lock().expect("stdout mutex");
    writeln!(out, "{value}").expect("write event");
    out.flush().expect("flush event");
}

fn value(args: &[String], name: &str) -> String {
    let index = args
        .iter()
        .position(|arg| arg == name)
        .expect("required argument");
    args[index + 1].clone()
}

fn command(session: &Session, v: &Value) -> Value {
    let path = v.get("path").and_then(Value::as_str).unwrap_or("main.tex");
    let text = v.get("text").and_then(Value::as_str).unwrap_or("");
    match v["cmd"].as_str().unwrap_or("") {
        "edit" => {
            let Some(start_byte) = v["start"].as_u64() else {
                return json!({"reply":"error","message":"bad start"});
            };
            let Some(end_byte) = v["end"].as_u64() else {
                return json!({"reply":"error","message":"bad end"});
            };
            match session.apply_edit(
                path,
                Edit {
                    start_byte: start_byte as usize,
                    end_byte: end_byte as usize,
                    text: text.into(),
                },
            ) {
                Ok(result) => json!({"reply":"edit","result":result}),
                Err(e) => json!({"reply":"error","message":e.to_string()}),
            }
        }
        "spans" => json!({"reply":"spans","spans":session.spans(path)}),
        "units" => json!({"reply":"units","units":session.layout_units()}),
        "set_document" => match session.set_document(path, text) {
            Ok(result) => json!({"reply":"set_document","result":result}),
            Err(e) => json!({"reply":"error","message":e.to_string()}),
        },
        "export_pdf" => {
            let out = v
                .get("out")
                .and_then(Value::as_str)
                .unwrap_or("/work/export.pdf");
            json!({"reply":"export_pdf","result":session.export_pdf(out)})
        }
        "status" => json!({"reply":"status","versions":session.versions(),
                           "convergence":session.convergence()}),
        "request_layout" => {
            session.request_layout();
            json!({"reply":"request_layout"})
        }
        _ => json!({"reply":"error","message":"unknown evaluation command"}),
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let mut cfg = SessionConfig::new(PathBuf::from(value(&args, "--project")), "main.tex");
    cfg.build_dir = PathBuf::from(value(&args, "--build"));
    cfg.pass_timeout = Duration::from_millis(value(&args, "--pass-timeout-ms").parse()?);
    let session = Arc::new(Session::open(cfg)?);
    let out = Arc::new(Mutex::new(std::io::stdout()));
    let (events, writer) = (session.clone(), out.clone());
    let running = Arc::new(AtomicBool::new(true));
    let polling = running.clone();
    let pump = std::thread::spawn(move || {
        while polling.load(Ordering::Relaxed) {
            for event in events.poll(Duration::from_millis(100)) {
                emit(
                    &writer,
                    &serde_json::to_value(event).expect("serialize event"),
                );
            }
        }
    });
    for line in std::io::stdin().lock().lines() {
        let v: Value = match serde_json::from_str(&line?) {
            Ok(v) => v,
            Err(e) => {
                emit(&out, &json!({"reply":"error","message":e.to_string()}));
                continue;
            }
        };
        if v["cmd"] == "quit" {
            break;
        }
        emit(&out, &command(&session, &v));
    }
    running.store(false, Ordering::Relaxed);
    pump.join().expect("event pump");
    Arc::try_unwrap(session)
        .ok()
        .expect("pump released session")
        .close();
    Ok(())
}
