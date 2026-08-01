import ballerina/io;
import ballerina/http;
import ballerina/time as clock;

# Grammar stress fixture: declarations, expressions, templates, XML, and regex.
# Unicode coverage includes BMP and astral text: café λ 東京 🚀 𝌆.
const string APPLICATION = "syntax-lab";
const int DEFAULT_LIMIT = 8;

enum Mode {
    BASIC,
    VERBOSE,
    TRACE
}

type Address record {|
    string city;
    string country = "JP";
    int postalCode?;
|};

type Person record {|
    readonly string id;
    string name;
    int age;
    Address address;
    string[] tags = [];
|};

type Result record {|
    string summary;
    int total;
    boolean accepted;
|};

type TextOrNumber string|int;
type Pair [string, int];

annotation string Trace on function;

@Trace {value: "construction"}
public function makePerson(string id, string name, int age = 21) returns Person {
    Address home = {
        city: "東京",
        country: "JP",
        postalCode: 1000001
    };
    return {
        id,
        name,
        age,
        address: home,
        tags: ["café", "λ", "🚀", "𝌆"]
    };
}

class Counter {
    private int value = 0;

    public function increment() returns int {
        self.value += 1;
        return self.value;
    }

    public function current() returns int => self.value;
}

public function classify(int value) returns string {
    if value < 0 {
        return "negative";
    } else if value == 0 {
        return "zero";
    } else {
        return "positive";
    }
}

function arithmetic(int left, int right) returns int {
    int sum = left + right;
    int difference = left - right;
    int product = left * right;
    int quotient = right == 0 ? 0 : left / right;
    int shifted = (sum << 1) | 1;
    return sum + difference + product + quotient + shifted;
}

function collectEven(int[] values) returns int[] {
    int[] selected = from int value in values
        where value % 2 == 0
        order by value descending
        limit DEFAULT_LIMIT
        select value;
    return selected;
}

function summarize(Person person, int[] scores) returns Result {
    int total = 0;
    foreach int score in scores {
        if score < 0 {
            continue;
        }
        total += score;
        if total > 100 {
            break;
        }
    }
    boolean accepted = total >= 50 && person.age >= 18;
    string summary = string `${person.name} from ${person.address.city}
scored ${total}; accepted=${accepted}`;
    return {summary, total, accepted};
}

function renderProfile(Person person, Result result) returns xml {
    xml profile = xml `<profile id="${person.id}" mode="verbose">
        <!-- Multiline XML comment: café λ
             astral symbols remain intact: 🚀 𝌆 -->
        <name>${person.name}</name>
        <city country="${person.address.country}">${person.address.city}</city>
        <summary accepted="${result.accepted}">${result.summary}</summary>
    </profile>`;
    return profile;
}

function templateSamples(string name, int count) returns string {
    string escaped = "quote=\" slash=\\ tab=\t unicode=\u{03bb}";
    string report = string `begin report for ${name}
line one: café λ 東京
line two: 🚀 𝌆
count: ${count}
escaped: ${escaped}
end report`;
    return report;
}

function regexSamples(string prefix) returns re {
    re pattern = re `^(?i:${prefix})-[A-Z\d]{2,8}\p{L}+$`;
    return pattern;
}

function mappingSamples() returns map<anydata> {
    map<anydata> values = {
        name: APPLICATION,
        enabled: true,
        attempts: 0x2A,
        ratio: 12.5,
        nested: {language: "ballerina", stable: false},
        coordinates: [35, 139]
    };
    return values;
}

function matchSample(TextOrNumber input) returns string {
    match input {
        string text => {
            return string `text:${text}`;
        }
        int number if number > 0 => {
            return string `positive:${number}`;
        }
        var other => {
            return string `other:${other}`;
        }
    }
}

function guardedDivision(int numerator, int denominator) returns int|error {
    if denominator == 0 {
        return error("division by zero", numerator = numerator);
    }
    return numerator / denominator;
}

function errorFlow(int denominator) returns int {
    int|error attempted = guardedDivision(84, denominator);
    if attempted is error {
        return -1;
    }
    int trapped = trap panic "illustrative panic";
    return attempted + trapped;
}

function transactionalSample() returns error? {
    transaction {
        check io:println("transaction body");
        commit;
    } on fail error reason {
        io:println("rolled back: ", reason.message());
    }
    return ();
}

function workerSample() returns int {
    worker left returns int {
        return 20;
    }
    worker right returns int {
        return 22;
    }
    int first = wait left;
    int second = wait right;
    return first + second;
}

function naturalSample(string topic) returns string {
    string prompt = natural {
        Explain ${topic} in two lines.
        Preserve the literal text café λ 東京 🚀 𝌆.
    };
    return prompt;
}

listener http:Listener endpoint = check new (9090);

service /syntax on endpoint {
    resource function get health() returns json {
        return {status: "ok", application: APPLICATION};
    }

    resource function get person/[string id]() returns Person {
        return makePerson(id, "Ada", 36);
    }

    remote function inspect(string value) returns string {
        return matchSample(value);
    }
}

public function main() returns error? {
    Person person = makePerson("p-λ", "Miyuki", 29);
    int[] scores = [12, 18, 24, 30];
    int[] even = collectEven(scores);
    Result result = summarize(person, even);
    xml profile = renderProfile(person, result);
    Counter counter = new;
    int first = counter.increment();
    Pair pair = [classify(first), arithmetic(7, 3)];
    map<anydata> metadata = mappingSamples();
    re identifier = regexSamples("item");
    string report = templateSamples(person.name, result.total);
    string generated = naturalSample("TextMate lexical states");
    io:println(profile, pair, metadata, identifier, report, generated);
    check transactionalSample();
    io:println("workers=", workerSample(), ", now=", clock:utcNow());
    return ();
}
