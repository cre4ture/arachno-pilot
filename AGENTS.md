# Workspace Instructions

- Follow DRY principles. Before adding new logic, check whether the same behavior already exists elsewhere in the workspace and reuse or extract it instead of duplicating it.
- Prefer moving shared behavior into an appropriate crate or helper module when it is needed by more than one app or component.
- We do test-driven development. New logic must be accompanied by tests, and tests should be written before or alongside the implementation.
