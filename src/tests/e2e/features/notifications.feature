Feature: Notification log

  Scenario: An application records a notification and the launcher works on it
    Given an application records a notification
    When I open the notification application
    Then the notification is listed, can be filtered, read and removed

  Scenario: A notification recorded while the launcher is open pops up as a toast
    Given the launcher is open
    And an application records a notification
    Then the notification pops up as a toast that opens the notification application
