Feature: Application shell

  As a user of the application
  I want the generated shell to answer
  so that I can build the first screens on top of it.

  Scenario: The application answers on the home page
    Given I open the application
    Then the page has a title
    And search engines are asked to stay away
