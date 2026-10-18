Feature: Gateway login
  Scenario: A session is required to open the console
    Given I have no gateway session
    When I open the console without signing in
    Then I am sent to the login page
    When I sign in through the gateway
    Then the console opens with a server session

  Scenario: A wrong password opens no session
    Given I have no gateway session
    When I go to "/login"
    Then "login-form" is visible
    When I fill "login-username" with "admin"
    And I fill "login-password" with "not the password"
    And I click on "login-submit"
    Then the sign-in is refused
    And the address ends with "/login"
