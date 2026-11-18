@id: feat:tags-and-search
@stratum: L0
@tag: read_query, mutable_state, persistent_storage
@substrate: DBTable:todos, Viewport:TodoList

Feature: Users can organize tasks with project tags, search keywords, and filter views
  As a user managing dozens of daily tasks
  I want to attach #tags to tasks and filter by tag, search query, or completion status
  So that I can quickly focus on a specific project or context

  Scenario: User filters todo board by project tag or keyword
    Given tasks tagged with "#work", "#rust", and "#home"
    When the user filters the board by tag "#rust" or search query "gvcs"
    Then only matching tasks are returned, sorted by their @native Urgency Score
