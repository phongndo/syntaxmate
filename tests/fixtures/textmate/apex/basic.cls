/**
 * @author Fixture Team
 * @usage `GreetingService.greet('Ada')`
 */
public with sharing class GreetingService {
    // BMP and astral text: café λ 東京 🚀 𝌆
    public static final String DEFAULT_NAME = 'world';

    @AuraEnabled(cacheable=true)
    public static String greet(
        String name
    ) {
        /* Keep this block comment
           deliberately multiline. */
        String chosen = name == null ? DEFAULT_NAME : name;
        List<Account> rows = [
            SELECT Id, Name FROM Account WHERE Name = :chosen LIMIT 1
        ];
        return 'Hello, ' + chosen + ' (' + rows.size() + ')';
    }
}
