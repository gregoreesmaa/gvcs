@id: feat:todo-crud
@stratum: L0
@tag: pii, user_data, mutable_state, persistent_storage, read_query
@substrate: DBTable:todos, Viewport:TodoList

Feature: Users can create, list, and complete todo tasks
  As a busy user
  I want to manage my personal todo items
  So that I can track my daily work

  Scenario: User creates a new todo task with an owner email
    Given an authenticated user with email "alice@example.com"
    When they create a todo task titled "Ship gvcs architecture"
    Then the task is stored in "DBTable:todos" with status Pending

  Scenario: User completes an existing todo task
    Given an existing pending task #1 owned by "alice@example.com"
    When they mark task #1 as completed
    Then the task status transitions to Completed
