Feature: Terminal
  Scenario: The terminal runs commands in a shell of its own and keeps its look
    Given the launcher is open
    When I open the terminal
    And I run "echo ciao-dal-nas" in the terminal
    Then the terminal shows "ciao-dal-nas"
    When I run "false" in the terminal
    Then the terminal shows "Uscito con codice 1."
    When I run "cd /tmp" in the terminal
    Then the prompt of the terminal is in "/tmp"
    When I open the system log of the terminal
    Then the log tab takes the place of the console
    When I make the terminal font bigger and save
    Then the terminal keeps the bigger font
