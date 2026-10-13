Feature: Application packages

  Scenario: Install an application from its repository and open it
    Given the catalogue offers an application
    When I install it from the application manager
    Then the package is fetched, unpacked and registered
    And updating it preserves the desktop choices
    And it opens in a frame served by its own process
    And it can ask the launcher for a notification
    And removing it takes the package and the row away
