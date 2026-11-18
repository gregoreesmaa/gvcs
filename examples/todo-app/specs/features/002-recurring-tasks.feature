Rollout: Canary 25% (flag_recurring_tasks)

Feature: Recurring tasks automatically spawn the next occurrence upon completion
  As a power user with repeating habits
  I want completed recurring tasks to schedule their next instance
  So that I don't have to re-enter weekly routines manually

  Scenario: Completing a recurring task spawns the next cycle
    Given a todo task with recurrence rule "Every 7 days"
    When the user marks the task as completed
    Then the current task is marked Completed
    And a new Pending task is automatically spawned for the next cycle
    Invariant: Completing an already-completed recurring task must NEVER spawn duplicate occurrences
