Feature: Applications on disk

  Scenario: A package copied into the directory is an installed application
    Given a package is copied into the applications directory
    Then the launcher lists it and serves the icon out of its package
    And taking the directory away takes the application with it
