Feature: First configuration after the wizard
  Scenario: The NAS installs its applications before the sign-in
    Given the wizard has just configured the NAS
    When I open the console without signing in
    Then the NAS is prepared before the sign-in
    And I am sent to the login page

  Scenario: A NAS the wizard has not configured says so
    Given the wizard has not configured the NAS
    When I open the console without signing in
    Then the page asks to run the wizard
