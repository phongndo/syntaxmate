/**
 * @name CodeQL TextMate stress fixture
 * @description Broad grammar coverage with BMP λ 東京 and astral 🚀 𝌆.
 * @kind path-problem
 * @problem.severity warning
 * @precision high
 * @id mark/codeql-stress
 * @tags maintainability external/cwe/cwe-000
 */
import semmle.code.cpp.dataflow.DataFlow
import semmle.code.cpp.controlflow.Guards as Guards
import semmle.code.cpp.rangeanalysis.SimpleRangeAnalysis
import cpp

pragma[noinline]
language[monotonicAggregates]
bindingset[left, right]
private predicate ordered(int left, int right) {
  left <= right and right >= left
}

newtype Severity = Low or Medium or High
newtype Outcome = Good = Success or Bad = Failure

module FixtureApi {
  signature predicate accepts(string text);
  signature string decorate(string text);
}

module FixtureImpl implements FixtureApi {
  predicate accepts(string text) {
    text != "" and not text = "ignored"
  }

  string decorate(string text) {
    result = "[" + text + "]"
  }
}

abstract class Finding extends DataFlow::Node {
  private string label;
  transient int cachedRank;

  Finding() {
    this.label = "finding" and
    this.cachedRank = 1
  }

  abstract string getKind();

  final string getLabel() {
    result = label
  }

  override string toString() {
    result = getKind() + ":" + getLabel()
  }
}

class SourceFinding extends Finding {
  SourceFinding() { this.getKind() = "source" }

  override string getKind() { result = "source" }
}

class SinkFinding extends Finding {
  SinkFinding() { this.getKind() = "sink" }

  override string getKind() { result = "sink" }
}

cached predicate sameLabel(Finding a, Finding b) {
  a.getLabel() = b.getLabel()
}

external predicate suppliedByExtractor(string key);

deprecated predicate oldCheck(int value) {
  value = 0
}

predicate numericLiterals(int value, float ratio) {
  value = -42 and ratio = 3.1415
}

predicate booleanLogic(boolean enabled, int value) {
  enabled = true and
  not false and
  (value < 10 or value > 20) and
  value != 13
}

predicate rangeAndMembership(int value, int lower, int upper) {
  value in [lower .. upper] and ordered(lower, upper)
}

predicate typeChecks(DataFlow::Node node) {
  node instanceof Finding and
  node.(Finding).getLabel() = "finding"
}

predicate existential(Finding finding) {
  exists(string text |
    text = finding.getLabel() and
    FixtureImpl::accepts(text)
  )
}

predicate universal(int limit) {
  forall(int i |
    i in [0 .. limit]
  |
    i <= limit
  )
}

predicate forexample(int limit) {
  forex(int i |
    i in [0 .. limit]
  |
    i = limit
  )
}

predicate noExamples() {
  none(int i | i < 0 | i = 0)
}

int aggregateCount(Finding finding) {
  result = count(string text | text = finding.getLabel())
}

int aggregateStrictCount(Finding finding) {
  result = strictcount(string text | text = finding.getLabel())
}

int aggregateSum(int limit) {
  result = sum(int i | i in [0 .. limit] | i)
}

int aggregateStrictSum(int limit) {
  result = strictsum(int i | i in [0 .. limit] | i)
}

float aggregateAverage(int limit) {
  result = avg(int i | i in [0 .. limit] | i)
}

int aggregateBounds(int limit) {
  result = min(int i | i in [0 .. limit] | i) +
    max(int j | j in [0 .. limit] | j)
}

string aggregateText(Finding finding) {
  result = concat(string text | text = finding.getLabel() | text, ",")
}

string strictAggregateText(Finding finding) {
  result = strictconcat(string text | text = finding.getLabel() | text, "|")
}

int aggregateRank(Finding finding) {
  result = rank[5](int score | score = aggregateCount(finding) | score)
}

predicate conditional(int value, string description) {
  description =
    if value = 0
    then "zero"
    else "non-zero"
}

query predicate exportedCheck(Finding finding) {
  existential(finding) implies finding.getLabel() != ""
}

/** Documentation tags continue across lines.
 * @note Escapes: \"quote\", \\slash, \n newline, \t tab.
 */
from Finding finding, int score, string note
where
  exportedCheck(finding) and
  score = aggregateCount(finding) and
  note = FixtureImpl::decorate("café 東京 🚀 𝌆")
select finding,
  note as message,
  score as result,
  finding.getKind() as kind
order by score desc, finding.getLabel() asc
