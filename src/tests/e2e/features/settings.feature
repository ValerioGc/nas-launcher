Feature: Settings application
  @slow
  Scenario: Change the sections of the settings against the mock backend
    Given the mock backend is available
    When I change the general settings and the widget refresh
    Then the network section shows the connection and its wireless networks

  Scenario: The factory reset is refused where the machine cannot do it
    Given the mock backend is available
    When I open the recovery section of the settings
    Then the factory reset says it is disabled on this installation

  Scenario: The factory reset asks for the password of the account
    Given the mock backend is available
    And the NAS can be reset but the gateway refuses the password
    When I open the recovery section of the settings
    And I confirm the factory reset with a wrong password
    Then the reset is not started and the password is asked again

  Scenario: The configuration goes out to an encrypted file and comes back
    Given the mock backend is available
    And the gateway checks the password as the NAS does
    When I open the recovery section of the settings
    And I export the configuration encrypted with a password for the file
    And I import the exported file
    Then the launcher receives the configuration it exported
