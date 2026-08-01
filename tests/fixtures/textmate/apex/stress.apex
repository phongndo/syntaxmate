// Apex grammar stress corpus: café λ 東京 🚀 𝌆
/**
 * Service used to review declarations, expressions, queries, and flow control.
 * @author Fixture Team
 * @since 2026
 * @param seed initial value
 * @return a normalized result
 * @throws FixtureException when a requested record cannot be found
 * @see `System.debug`
 */
@SuppressWarnings('PMD.AvoidGlobalModifier')
global inherited sharing virtual class GrammarService
        extends BaseGrammarService
        implements Queueable, Schedulable {
    public static final String UNICODE = 'café λ 東京 🚀 𝌆';
    private static final Integer MAX_ATTEMPTS = 3;
    private transient Map<Id, List<Account>> accountCache;
    protected Decimal ratio = 12.50M;
    global Boolean Enabled { get; private set; }
    public String Label {
        get {
            return label == null ? 'unset' : label;
        }
        set {
            label = value == null ? UNICODE : value.trim();
        }
    }

    /// <summary lang="en">Builds a result &amp; records diagnostics.</summary>
    /// <![CDATA[The comparison 1 < 2 remains readable.]]>
    public GrammarService() {
        this(new Map<Id, List<Account>>());
    }

    public GrammarService(Map<Id, List<Account>> initialCache) {
        this.accountCache = initialCache;
        this.Enabled = true;
    }

    public override void execute(QueueableContext context) {
        processAccounts(new Set<String>{ 'Acme', '東京', 'café' });
    }

    global void execute(SchedulableContext context) {
        System.enqueueJob(this);
    }

    @TestVisible
    private static List<Account> processAccounts(Set<String> names) {
        /* This comment crosses lines so the tokenizer must preserve
           a block-comment state until the explicit terminator. */
        List<Account> rows = [
            SELECT Id,
                   Name,
                   Owner.Name,
                   AnnualRevenue,
                   (SELECT Id, LastName FROM Contacts WHERE CreatedDate = LAST_N_DAYS:30)
            FROM Account
            WHERE Name IN :names
              AND LastModifiedDate = THIS_YEAR
            ORDER BY Name ASC NULLS LAST
            LIMIT 200
        ];

        Map<Id, Account> byId = new Map<Id, Account>(rows);
        List<Account> changed = new List<Account>();
        Integer index = 0;
        Long mask = 0x2AL;
        Integer binary = 0b1010;
        Double scientific = 6.02E23D;
        Date literalDate = 2026-07-12;
        Datetime literalTime = 2026-07-12T10:30:45.125Z;

        for (Account row : rows) {
            String ownerName = row.Owner?.Name;
            Boolean usable = row.Name != null && !row.Name.startsWith('X');
            if (usable && (index++ < MAX_ATTEMPTS || ownerName == null)) {
                row.Description = 'Owner: ' + (ownerName ?? 'none');
                row.AnnualRevenue = row.AnnualRevenue == null
                    ? 0.0
                    : row.AnnualRevenue * ratio;
                changed.add(row);
            } else if (row.Name == 'skip') {
                continue;
            } else {
                break;
            }
        }

        for (Integer i = 0; i < changed.size(); i++) {
            mask <<= 1;
            binary += i % 2;
        }

        Integer attempts = 0;
        while (attempts < MAX_ATTEMPTS) {
            attempts++;
        }
        do {
            attempts--;
        } while (attempts > 0);

        switch on changed.size() {
            when 0 {
                System.debug(LoggingLevel.WARN, 'No rows');
            }
            when 1, 2 {
                System.debug('Small batch');
            }
            when else {
                System.debug('Large batch: ' + changed.size());
            }
        }

        Savepoint checkpoint = Database.setSavepoint();
        try {
            if (!changed.isEmpty()) {
                update changed;
            }
            Account inserted = new Account(
                Name = 'Inserted café',
                Description = UNICODE
            );
            insert inserted;
            Account duplicate = new Account(Name = inserted.Name);
            merge inserted duplicate;
            upsert changed Account.External_Id__c;
        } catch (DmlException error) {
            Database.rollback(checkpoint);
            throw new FixtureException('DML failed: ' + error.getMessage());
        } finally {
            System.debug('processed=' + changed.size());
        }

        return changed;
    }

    public static Map<String, Object> expressionGallery(Object input) {
        Integer[] values = new Integer[]{ 1, 2, 3 };
        Integer first = values[0];
        Integer shifted = (first << 2) | 0x1;
        Boolean flags = true && false || !false;
        String escaped = 'quote: \' slash: \\ newline: \n';
        Object casted = (String) input;
        Map<String, Object> result = new Map<String, Object>{
            'first' => first,
            'shifted' => shifted,
            'flags' => flags,
            'escaped' => escaped,
            'casted' => casted,
            'missing' => null
        };
        result.put('conditional', flags ? 'yes' : 'no');
        return result;
    }

    public static AggregateResult[] aggregate(String prefix) {
        return [
            SELECT OwnerId,
                   COUNT(Id) total,
                   SUM(AnnualRevenue) revenue
            FROM Account
            WHERE Name LIKE :prefix
            GROUP BY OwnerId
            HAVING COUNT(Id) > 0
            ORDER BY COUNT(Id) DESC
        ];
    }

    public virtual class FixtureException extends Exception {
    }
}

public interface NamedFixture<T> {
    T transform(T value);
    String Name { get; }
}

public enum FixtureState {
    NEW,
    RUNNING,
    DONE
}

trigger GrammarAccountTrigger on Account (
    before insert,
    before update,
    after delete,
    after undelete
) {
    if (Trigger.isBefore && (Trigger.isInsert || Trigger.isUpdate)) {
        for (Account row : Trigger.new) {
            row.Description = row.Description == null
                ? 'trigger 🚀'
                : row.Description;
        }
    }
    if (Trigger.isAfter && Trigger.isDelete) {
        System.debug('deleted=' + Trigger.oldMap.keySet());
    }
}
