Feature: Ongoing operations

  Scenario: An application registers an operation and the widget drives it
    Given an application registers a long operation
    When I open the operations widget
    Then the operation is shown, can be paused and can be swept up when it ends
