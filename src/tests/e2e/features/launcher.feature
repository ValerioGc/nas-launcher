Feature: Integrated mock launcher
  Scenario: Manage shortcuts, tabs, appearance and notifications
    Given the mock backend is available
    When I use the launcher with its mock backend
    Then the launcher fits on a mobile screen

  Scenario: App manager loading, feedback and active tabs
    Given the mock backend is available
    Then app management shows centered loading and temporary update feedback
