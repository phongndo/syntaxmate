use v6.d;
use JSON::Fast;

=begin pod
Telemetry console for a mountain observatory.
The documentation mentions café, λ, 東京, and an astral satellite 🛰.
Every POD, quote, regex, and code delimiter closes before EOF.
=end pod

# Configuration values and numeric/operator forms.
constant SAMPLE-LIMIT = 1_024;
my Rat $warning-ratio = 3/4;
my Num $epsilon = 6.022e-23;
my Complex $calibration = 2.5+1.25i;
my $not-a-number = NaN;
my $unbounded = Inf;
my Bool $verbose = True;

enum Severity <info warning critical>;
subset Percentage of Int where 0 <= * <= 100;

role Timestamped {
    has DateTime $.observed-at is required;
    method age(Instant $now = now) returns Duration {
        $now - $!observed-at.Instant
    }
}

class Reading does Timestamped {
    has Str $.sensor is required;
    has Real $.value is rw;
    has Str $.unit = 'raw';
    has Severity $.severity = info;

    method gist() returns Str {
        "{$!sensor}: {$!value} {$!unit} ({$!severity})"
    }

    method normalized(Real $low, Real $high where * > $low) {
        (($!value - $low) / ($high - $low)).max(0).min(1)
    }

    submethod BUILD(:$!sensor, :$!value, :$!unit, :$!severity,
                    :$!observed-at = DateTime.now) {
        die "empty sensor" unless $!sensor.chars;
    }
}

class Station {
    has Str $.name is required;
    has @.readings handles <push elems>;
    has %.metadata is rw;
    state $instances = 0;

    method new(*%args) {
        $instances++;
        self.bless(|%args)
    }

    multi method add(Reading $reading) {
        @!readings.push($reading);
        self
    }

    multi method add(Str $sensor, Real $value, Str $unit = 'raw') {
        self.add(Reading.new(:$sensor, :$value, :$unit))
    }

    method report(:$color = False) {
        @!readings.map(*.gist).join("\n")
    }
}

grammar ReadingLine {
    token TOP { ^ <sensor> ':' \s* <value> \s* <unit>? $ }
    token sensor { <[A..Za..z]>+ [ '-' <[A..Za..z0..9]>+ ]* }
    token value { <[+\-]>? \d+ [ '.' \d+ ]? }
    token unit { '%' | '°C' | 'm/s' | 'arcsec' }
}

proto sub classify(Real --> Severity) {*}
multi sub classify(Real $value where * < 0 --> Severity) { critical }
multi sub classify(Real $value where 0 <= * < 80 --> Severity) { info }
multi sub classify(Real $value --> Severity) { warning }

sub parse-reading(Str:D $line --> Reading) {
    my $match = ReadingLine.parse($line);
    fail "cannot parse '$line'" without $match;
    my $value = +$match<value>;
    Reading.new(
        sensor => ~$match<sensor>,
        value => $value,
        unit => ~$match<unit> || 'raw',
        severity => classify($value),
    )
}

# Single, double, generalized, nested, and heredoc-style quotes.
my Str $plain = 'literal backslash \\ and quote \' stay contained';
my Str $welcome = "Welcome, José — station Δ says 🚀\n";
my Str $braced = q{single quoted {with nested braces}};
my Str $interpolated = qq{{station={$welcome.trim}; limit={SAMPLE-LIMIT}}};
my Str $parenthesized = Q((literal (nested) $welcome));
my Str $bracketed = q[alpha [beta] gamma];
my @words = Q:w<temperature humidity wind seeing>;

my Str $instructions = q:to/END-INSTRUCTIONS/;
Enter readings as “sensor: value unit”.
Unicode examples: température: 18.5 °C and sky: 1.2 arcsec 🪐
Blank lines and comments are ignored.
END-INSTRUCTIONS

my @raw-lines =
    'temperature: 18.5 °C',
    'humidity: 62 %',
    'wind: 4.2 m/s',
    'seeing: 1.1 arcsec',
    'bad input';

my $station = Station.new(
    name => 'North Ridge',
    metadata => {
        operator => 'Zoë',
        coordinates => [48.8566, 2.3522],
        active => True,
    },
);

for @raw-lines.kv -> $index, $line {
    next if $line.trim eq '';
    try {
        my $reading = parse-reading($line);
        $station.add($reading);
        say "accepted #{$index + 1}: {$reading.gist}" if $verbose;
        CATCH {
            when X::AdHoc {
                warn "rejected '$line': {.message}";
            }
            default {
                note "unexpected parser failure: {.gist}";
            }
        }
    }
}

my @safe = gather {
    for $station.readings -> $reading {
        take $reading if $reading.value >= 0;
    }
}

my %by-unit = @safe.classify(*.unit);
my %averages = %by-unit.map: -> $pair {
    my $unit = $pair.key;
    my @group = $pair.value.List;
    $unit => @group.map(*.value).sum / @group.elems
};

given @safe.elems {
    when 0 {
        say 'No usable readings';
    }
    when 1 {
        say 'A single reading arrived';
    }
    default {
        say "Processed $_ readings across {%by-unit.elems} units";
    }
}

my $attempt = 0;
repeat {
    $attempt++;
} while $attempt < 2;

loop (my $tick = 0; $tick < 3; $tick++) {
    next if $tick == 1;
    last if $tick > SAMPLE-LIMIT;
    $station.metadata<last-tick> = $tick;
}

while $warning-ratio < 1 {
    $warning-ratio += 1/8;
    last when $warning-ratio >= 1;
}

unless $station.elems > SAMPLE-LIMIT {
    my $joined = @safe.map(*.sensor).sort.join(', ');
    say "Sensors: $joined";
}

BEGIN {
    # Compile-time hook kept deliberately quiet.
    my $phase = 'compile';
}

CHECK {
    die 'sample limit must be positive' if SAMPLE-LIMIT <= 0;
}

END {
    note 'telemetry fixture complete' if $verbose;
}

say $instructions;
say $station.report;
say "Averages: " ~ to-json(%averages, :sorted-keys);
say "Comparison: " ~ (2 div 1) ~ ', ' ~ (5 mod 2);
say 'Logical path selected' if ($verbose and @safe) or $unbounded == Inf;
