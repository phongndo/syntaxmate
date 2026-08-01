use v6.d;

# A compact Unicode welcome: café λ 🚀
class Guest {
    has Str $.name is required;
    has Int $.visits is rw = 0;
    method greet(Str $punctuation = '!') returns Str {
        "Hello, {$!name}$punctuation — 世界 🌍"
    }
}

my $guest = Guest.new(name => 'Mira');
my @steps = 1, 2, 3;
my %labels = start => q{ready}, finish => qq{{done}};
for @steps -> $step {
    say "$step: " ~ $guest.greet;
}
say %labels<finish> if @steps.elems >= 3;
