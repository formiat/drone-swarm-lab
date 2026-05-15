# Startup Context

At the beginning of each new chat for this repository, load the local Claude and OpenCode context before doing substantive work.

1. Determine the absolute path of the repository root.
2. Compute the Claude project key from that absolute path by replacing every `/` with `-`.
   Example: `/home/formi/Documents/RustProjects/drone` -> `-home-formi-Documents-RustProjects-drone`.
3. Read the project memory file at `$HOME/.claude/projects/<project_key>/memory/MEMORY.md`.
4. Read the Claude Code skills directory at `$HOME/.claude/skills/`.
5. Read the Claude Code commands directory at `$HOME/.claude/commands/`.
6. Read the OpenCode skills directory at `$HOME/.config/opencode/skills/`.
7. Read the OpenCode commands directory at `$HOME/.config/opencode/commands/`.
8. Read every `CLAUDE.md` file inside this repository, including the root file and all nested files.

Implementation notes:

- Prefer discovering repository-local `CLAUDE.md` files from the repo root via `rg --files -g 'CLAUDE.md'`.
- If any of the Claude-specific paths under `$HOME/.claude/` are missing, note that briefly and continue with the available context.
- If any of the OpenCode-specific paths under `$HOME/.config/opencode/` are missing, note that briefly and continue with the available context.
- Treat this startup read as required context gathering, not as an optional hint.

## Test Expectations

For any planning, investigation, implementation, or test-planning workflow that touches code or prepares code changes, use these rules:

- When developing new functionality, aim to cover the new behavior with automated tests as much as practical.
- When changing existing functionality, either add new tests if the current coverage is insufficient or adapt the existing tests to the new logic.
- Tests must be self-contained and portable. Do not make automated tests depend on machine-specific local files, directories, or preexisting state such as local DB files, `$HOME` content, absolute paths, sibling repositories, mounted volumes, temp directories outside the test's control, or filesystem discovery that varies by machine. Prefer in-memory data, inline fixtures, and mock/fake/stub dependencies. If a file fixture is truly unavoidable, first rule out inline/in-memory alternatives, explain why, and use only small stable fixtures stored in the repository.
- Manual verification can complement tests, but it should not replace reasonable automated coverage when such coverage is feasible.
- In planning and test-planning artifacts, propose the maximum practical set of automated tests for the new and affected functionality. Group all proposed automated tests into three categories:
  1. Tests that need no refactoring — mark as planned for implementation together with the main functional changes.
  2. Tests that need light refactoring.
  3. Tests that need heavy refactoring.
  List all three categories explicitly. In investigations and plans, call out relevant existing tests, coverage gaps, and what tests implementation should add or update.
