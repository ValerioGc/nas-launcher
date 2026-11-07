Feature: Global search
  Scenario: Files are searched only by an application that provides the search
    Given I open the search application
    When I click on "search_scope_files"
    And I reset the file search filters
    Then the query is preserved and the launcher does not handle file searches

  Scenario: The table of files fits the search without scrolling sideways
    Given I open the search application
    And an application provides the search with long file names
    When I click on "search_scope_files"
    And I fill "search_query" with "report"
    Then the table of files fits its width

  Scenario: A settings entry opens its section
    Given I open the search application
    When I fill "search_query" with "pianificate"
    And I click on "search_entry_setting:power:powerRules"
    Then "power_rule_open" is visible
