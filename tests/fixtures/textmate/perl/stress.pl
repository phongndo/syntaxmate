#!/usr/bin/env perl
use v5.34;
use strict;
use warnings;
use utf8;
use feature qw(signatures say state switch);
no warnings qw(experimental::signatures experimental::smartmatch);
use Scalar::Util qw(blessed);
# Stress fixture: BMP naïve/λ/雪 and astral 🚀/𝄞/😀 in a comment.
package Fixture::Perl::Ledger;
our $VERSION = '2.1';
our @EXPORT_OK = qw(total describe);
my $next_id = 0;
BEGIN { our $loaded_from = __FILE__ }
CHECK { our $checked_at = __LINE__ }
INIT  { our $initialized = 1 }
END   { our $finished = 1 }
sub new ($class, %args) {
    state $created = 0;
    my $self = {
        id      => ++$next_id,
        owner   => $args{owner} // 'anonymous',
        entries => $args{entries} // [],
        serial  => ++$created,
    };
    return bless $self, $class;
}
sub add ($self, $label, $amount = 0) {
    push $self->{entries}->@*, { label => $label, amount => $amount };
    return $self;
}
sub total ($self) {
    my $sum = 0;
    $sum += $_->{amount} for $self->{entries}->@*;
    return $sum;
}
sub describe ($self, $prefix = 'ledger') {
    return sprintf '%s #%d for %s: %.2f',
        $prefix, $self->{id}, $self->{owner}, $self->total;
}
no feature 'signatures';
sub legacy_pair ($$) {
    my ($left, $right) = @_;
    return "$left=$right";
}
use feature 'signatures';
=pod

=head1 NAME

Fixture::Perl::Ledger - a B<hand-written> syntax fixture

=head2 DESCRIPTION

Exercises C<variables>, I<operators>, L<https://www.perl.org/>, and E<sol>.

=over 4

=item *

Unicode prose includes café λ 雪 and 🚀 𝄞 😀.

=back

=begin html

<p class="fixture">Embedded POD HTML is closed.</p>

=end html

=cut
package main;

my $owner = "Zoë λ 🚀";
my $escaped = "line\n\ttab \x{03BB} \N{SNOWMAN} \o{101}";
my $literal = 'single quoted \\' . q{ text};
my $nested_q = q{literal {nested} braces};
my $nested_qq = qq[owner=[$owner], music=𝄞];
my @words = qw<alpha beta café 😀>;
my $shell_text = qx{printf qx};
my $backtick_text = `printf backtick`;
my @numbers = (1, 2, 3, 5, 8);
my %weights = (alpha => 1, beta => 2, gamma => 3);
my $last_index = $#numbers;
my $count = scalar @numbers;
my $program = $0;
my $pid = $$;
my $interpreter = $^X;
my $status = $?;
my $error = "$!";
local $_ = 'alpha-42-omega';
local $/ = "\n";
local $\ = undef;
local $" = ', ';
our $qualified = 'visible';
my $copy = ${main::qualified};

if (/([a-z]+)-(\d+)-(?<tail>[a-z]+)/) {
    my ($first, $digits) = ($1, $2);
    my ($whole, $before, $after, $last) = ($&, $`, $', $+);
    my %named = %+;
    say join ':', $first, $digits, $named{tail}, $whole;
}

my $word = 'alpha';
my $compiled = qr{^(?:$word|beta)\s+\p{Letter}+$}iu;
my $paired = qr[(one|two)\[(?<value>\d+)\]];
my $matched = 'alpha λ' =~ m{$compiled};
my $slash_match = 'abc123' =~ /[a-z]+\d+$/;
my $sentence = 'old old value';
my $replacement = 'new';
$sentence =~ s{old}{$replacement}g;
$sentence =~ s/ \s+ / /gx;
$sentence =~ tr/a-z/A-Z/;
$sentence =~ y/ /_/;

my $raw = <<'RAW_TEXT';
$owner is not interpolated; literal café λ 🚀 𝄞.
Backslashes such as \n remain visible.
RAW_TEXT

my $interpolated = <<"MESSAGE";
Owner $owner has @numbers and escape \x{2603}.
The heredoc contains BMP 雪 and astral 😀.
MESSAGE

my $indented = <<~'INDENTED';
    first indented line
      nested indentation remains
    INDENTED

my $html = <<'HTML';
<section class="ledger">
  <h1>Fixture</h1>
</section>
HTML

my $sql = <<"SQL";
SELECT owner, amount
FROM entries
WHERE owner = '$owner';
SQL

my $ledger = Fixture::Perl::Ledger->new(
    owner => $owner,
    entries => [],
);
$ledger->add('books', 12.50)->add('music', 7.25);

my $array_ref = \@numbers;
my $hash_ref = \%weights;
my $code_ref = sub ($value) { return $value * 2 };
my @doubled = map { $code_ref->($_) } @$array_ref;
my @odd = grep { $_ % 2 } @doubled;
my @sorted = sort { $b <=> $a } @odd;
my $alpha_weight = $hash_ref->{alpha};
my @weight_names = sort keys %$hash_ref;
my $weight_total = 0;
$weight_total += $_ for values %$hash_ref;

for my $number (@numbers) {
    next if $number == 2;
    redo if 0;
    last if $number > 10;
    $weight_total += $number;
}

my $cursor = 0;
while ($cursor < 2) {
    $cursor++;
} continue {
    $status = $cursor <=> 1;
}

until ($cursor >= 4) {
    ++$cursor;
}

given (my $mode = 'sum') {
    when ('sum') { say $ledger->total }
    when (/list/) { say join ',', @weight_names }
    default { warn "unknown mode: $mode" }
}

my $comparison = ($owner cmp 'Zoe') || ($count <=> 3);
my $logical = defined($owner) && length($owner) > 0;
my $wordy = ($logical and not $comparison) or $status = 1;
my $exclusive = ($logical xor !$matched);
my $range_text = join '-', ('a' .. 'd');
my $repeated = ('ab' x 3) . ':' . (2 ** 4);
my $shifted = (16 >> 2) + (1 << 3);
my $mask = ($shifted & 0xff) | 0x10;
my $choice = $exclusive ? 'yes' : 'no';

my $exists = exists $weights{beta};
my $removed = delete $weights{gamma};
push @numbers, 13;
unshift @numbers, 0;
my $tail = pop @numbers;
my $head = shift @numbers;
my @middle = splice @numbers, 1, 2, (21, 34);
my ($key, $value) = each %weights;
my @pieces = split /:/, legacy_pair('left', 'right');
my $joined = join ',', reverse @pieces;
my $slice = substr $joined, 0, 4;

if (-e __FILE__ and -r __FILE__) {
    open my $handle, '<:encoding(UTF-8)', __FILE__ or die $!;
    my $first_line = <$handle>;
    chomp $first_line;
    close $handle or warn $!;
}

my $safe = eval { require 5.020; $ledger->describe('total') };
if (my $exception = $@) {
    warn "evaluation failed: $exception";
} elsif (blessed($ledger)) {
    say $safe unless $ENV{PERL_FIXTURE_QUIET};
} else {
    die 'not an object';
}

our $report_name = 'Ada';
format FIXTURE_REPORT =
Name: @<<<<<<<<<<<<<<<
$report_name
.

goto FINISH if @ARGV && $ARGV[0] eq '--quiet';
say "constants: ", __PACKAGE__, ' ', __SUB__, ' ', __LINE__;
FINISH:
say "summary: owner=$owner total=", $ledger->total;
