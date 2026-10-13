Feature: Application catalogue
  Scenario: The catalogue is edited from the application manager
    Given the launcher is open
    When I add an application to the catalogue from the manager settings
    Then the new application is on offer
    When I remove it from the catalogue
    Then it is no longer on offer
