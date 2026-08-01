import ballerina/io;

# A compact greeting fixture with Unicode: café λ 東京 🚀 𝌆.
type Greeting record {|
    string text;
    int count;
|};

public function main() {
    Greeting greeting = {text: "café λ 東京 🚀 𝌆", count: 3};
    int total = 0;
    foreach int item in [1, 2, 3] {
        total += item;
    }
    string message = string `Greeting: ${greeting.text}
count=${greeting.count}, total=${total}`;
    xml card = xml `<card lang="λ">
        <!-- 東京 is intentionally inside a multiline XML template. -->
        <message>${message}</message>
    </card>`;
    if total >= greeting.count && greeting.text != "" {
        io:println(message, card);
    }
}
