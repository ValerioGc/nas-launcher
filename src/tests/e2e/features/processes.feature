Feature: Application process termination
  Scenario Outline: Terminating an application removes its loaded interface
    Then the launcher closes the app view after terminating it from the "<source>"

    Examples:
      | source |
      | app    |
      | widget |
