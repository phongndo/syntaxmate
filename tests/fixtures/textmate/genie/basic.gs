[indent=4]

// Small Genie program: café, 東京, 🚀, 𝌆.
const DEFAULT_NAME:string = "world"

class Greeter:Object
    prop name:string

    construct (name:string)
        self.name = name

    def greet (times:int):string
        var message = @"Hello, $name!\n"
        for i:int = 1 to times
            print "%d: %s", i, message
        return message

init
    var greeter = new Greeter(DEFAULT_NAME)
    if greeter.name is not null and greeter.name != ""
        greeter.greet(2)
    else
        print "Nobody to greet"
