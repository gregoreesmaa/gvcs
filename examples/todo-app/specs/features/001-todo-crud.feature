Feature: Users can create, list, and complete todo tasks
  As a busy user
  I want to manage my personal todo items
  So that I can track my daily work

  Scenario: User creates a new todo task
    Given the Todo workspace is initialized
    When the user creates a todo task titled "Ship gvcs architecture"
    Then the task is saved in their todo list with status Pending

  Scenario: User completes an existing todo task
    Given an existing pending task #1
    When the user marks task #1 as completed
    Then the task status transitions to Completed
