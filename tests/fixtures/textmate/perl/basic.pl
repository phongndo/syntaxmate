#!/usr/bin/env perl
use v5.34;
use utf8;
use feature 'signatures';
no warnings 'experimental::signatures';

# A compact greeting with BMP café/λ and astral 🚀/𝄞 characters.
my $name = "Ada λ 🚀";
my @languages = qw(Perl Raku shell);
my %score = (Perl => 10, Raku => 8);

sub greeting ($who, $punctuation = '!') {
    return qq{Hello, $who$punctuation};
}

for my $language (@languages) {
    next unless exists $score{$language};
    say greeting("$name uses $language", '.');
}

my $line = <<'TEXT';
Literal café λ and 🚀 𝄞 stay together.
TEXT

$line =~ s/\s+/ /g;
say $line if $line =~ m{Perl|café};
