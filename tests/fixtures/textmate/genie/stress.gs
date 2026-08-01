[indent=4]

uses GLib

/*
 A reviewable Genie fixture covering declarations, flow, and literals.
 Unicode appears in both planes: café, 東京, 🚀, 𝌆.
 All comments and strings are deliberately closed.
*/
namespace Observatory
    const APP_TITLE:string = "Night Watch"
    const MAX_SAMPLES:int = 12
    const HEX_MASK:uint = 0x00FFu

    enum Phase
        IDLE
        PREPARING
        ACTIVE
        COMPLETE

    errordomain ObservatoryError
        BAD_READING
        CANCELLED

    delegate Formatter (value:double):string

    interface Reportable:Object
        def abstract report ():string

    struct Coordinates
        public latitude:double
        public longitude:double

        def describe ():string
            return "%.3f, %.3f".printf(latitude, longitude)

    abstract class Instrument:Object implements Reportable
        prop public name:string
        prop protected phase:Phase
        prop readonly serial:uint64
        prop volatile enabled:bool
        event changed (message:string)

        construct (name:string, serial:uint64)
            self.name = name
            self.serial = serial
            phase = Phase.IDLE
            enabled = true

        init
            changed("initialized")

        def protected virtual sample ():double
            return 0.0

        def abstract report ():string

    final class Telescope:Instrument
        private aperture:double = 2.4
        private readings:list of double = new list of double
        private labels:dict of string, string = new dict of string, string

        construct (name:string, serial:uint64)
            super(name, serial)
            labels["site"] = "東京"
            labels["operator"] = "café team"

        construct portable (name:string)
            super(name, 1UL)
            aperture = 0.8

        def protected override sample ():double
            var base_value = 41.5
            return base_value + aperture / 2.0

        def public collect (count:int = 3):array of double
            requires count > 0
            ensures result.length > 0
            var values = new array of double[count]
            phase = Phase.PREPARING
            for i:int = 0 to count - 1
                values[i] = sample() + i * 0.25
                readings.add(values[i])
            phase = Phase.ACTIVE
            changed(@"collected $(values.length) samples")
            return values

        def public report ():string
            var site = labels["site"]
            return @"$name at $site: $(readings.size) readings"

        def public diagnostics ():string
            var escaped = "quote=\" slash=\\ tab=\t newline=\n"
            var character:unichar = 'é'
            var line_break:char = '\n'
            var banner = """
Night Watch diagnostics
BMP: café and 東京
Astral: 🚀 and 𝌆
Operators shown as text: + - * / % == != <= >=
"""
            return "%s%c%c%s".printf(escaped, character, line_break, banner)

        def public matches_code (candidate:string):bool
            var code_pattern = /^[A-Z]{2}\/[0-9]{3}$/
            return code_pattern.match(candidate)

    class Session:Object
        private instrument:Instrument
        private formatter:Formatter
        private total:int = 0

        construct (instrument:Instrument)
            self.instrument = instrument
            formatter = def (value:double):string
                return "%.2f".printf(value)

        def run ():int raises ObservatoryError
            if instrument is Telescope
                var telescope = instrument as Telescope
                var samples = telescope.collect(4)
                for value in samples
                    if value < 0.0
                        raise new ObservatoryError.BAD_READING("negative sample")
                    else if value == 0.0
                        continue
                    else
                        total += (int) value
                    print formatter(value)
            else
                raise new ObservatoryError.CANCELLED("unsupported instrument")
            return total

        def classify ():string
            case total
                when 0
                    return "empty"
                when 1, 2, 3
                    return "small"
                default
                    return "large"

        def operator_gallery (input:int):int
            var mask = (input << 2) | 0x0F
            mask &= 0xFF
            mask ^= 0x55
            if not (mask >= 0 and mask <= 255) or input < 0
                return ~mask
            return mask % 17

        def loop_gallery (limit:int)
            var cursor = 0
            while cursor < limit
                cursor++
                if cursor == 2
                    pass
                else if cursor > MAX_SAMPLES
                    break
            do
                cursor--
            while cursor > 0
            for reverse:int = 3 downto 1
                total += reverse

        def guarded_update (amount:int)
            lock self
                total += amount
            assert total >= 0

        def safe_run ():int
            try
                return run()
            except error:ObservatoryError
                stderr.printf("session failed: %s\n", error.message)
                return -1
            finally
                instrument.changed("session closed")

    def static numeric_gallery ():double
        var decimal:int64 = 1_000L
        var tiny:float = .5f
        var scientific:double = 6.022e23
        var unsigned_value:ulong = 42UL
        return decimal + tiny + scientific + unsigned_value

    def static container_gallery ()
        var names:list of string = new list of string
        var lookup:dict of string, int = new dict of string, int
        var primes:array of int = {2, 3, 5, 7}
        names.add("café")
        names.add("東京")
        lookup["rocket"] = primes[2]
        for name in names
            print @"name=$name value=$(lookup[\"rocket\"])"

    def static main ()
        var scope = "outer"
        var telescope = new Telescope("Asteria 🚀 𝌆", 9001UL)
        telescope.changed += def (message:string)
            print @"event: $message"
        var session = new Session(telescope)
        var status = session.safe_run()
        session.loop_gallery(4)
        session.guarded_update(1)
        print telescope.report()
        print telescope.diagnostics()
        print session.classify(), session.operator_gallery(status)
        container_gallery()
        print numeric_gallery(), scope

#if DEBUG
        print "debug build"
#elif TESTING
        print "testing build"
#else
        print "release build"
#endif

init
    Observatory.main()
