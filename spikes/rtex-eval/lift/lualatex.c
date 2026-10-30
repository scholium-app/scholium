/*
 * Evaluation-only LuaLaTeX launcher for the T16 E2 negative controls (never product code).
 * rtex runs `$RTEX_TEXLIVE_BIN/lualatex` without --shell-escape, so LuaTeX itself refuses
 * arbitrary commands and does not load LuaSocket. This launcher adds --shell-escape (which
 * also enables LuaSocket) to model a host where that TeX-level restriction is gone, so the
 * shell-escape and network probes meet the bubblewrap sandbox instead of LuaTeX.
 * It is compiled to a native binary because the sandbox deliberately has no /bin/sh.
 */
#include <stdlib.h>
#include <unistd.h>

int main(int argc, char **argv) {
  char **args = calloc((size_t)argc + 2, sizeof *args);
  if (args == NULL) {
    return 127;
  }
  /* An absolute argv[0] keeps kpathsea's SELFAUTOLOC at /usr/bin and selects the lualatex format. */
  args[0] = "/usr/bin/lualatex";
  args[1] = "--shell-escape";
  for (int i = 1; i < argc; i++) {
    args[i + 1] = argv[i];
  }
  execv("/usr/bin/lualatex", args);
  return 127;
}
