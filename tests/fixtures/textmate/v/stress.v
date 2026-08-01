module fixtures

import encoding.json
import math as m
import os
import strings

// Tier-B V stress fixture: café λ 東京 🚀 𝌆
/* Outer documentation block starts here.
   It deliberately spans several physical lines.
   /* The grammar also recognizes this nested comment. */
   All comment delimiters are closed before code resumes. */

#flag -D fixture_mode
#include <stdio.h>

[deprecated]
pub type UserId u64
type Callback fn (string) bool
type StringMap map[string]string

pub enum Phase {
	queued
	running
	done
}

[flag]
enum Permission {
	read
	write
	execute
}

pub interface Displayable {
	display() string
}

interface Reader {
	read(mut buffer []byte) !int
}

[heap]
pub struct Job {
pub:
	id UserId
	title string
	phase Phase = .queued
mut:
	tags []string
	metadata map[string]string
}

union NumberBits {
	i i64
	f f64
}

struct Point {
	x f64
	y f64
}

const (
	answer = 42
	mask = 0xff_ff
	binary = 0b1010_0110
	octal = 0o755
	decimal = 1_000_000
	ratio = 3.141_592
	exponent = 6.02e+23
)

__global global_jobs []Job

pub fn new_job(id UserId, title string) Job {
	return Job{
		id: id
		title: title
		tags: ['new', 'café', '東京']
		metadata: {
			'rocket': '🚀'
			'tetragram': '𝌆'
		}
	}
}

pub fn identity<T>(value T) T {
	return value
}

fn (job Job) display() string {
	return '${job.id}: ${job.title} [${job.phase}]'
}

fn (mut job Job) add_tag(tag string) {
	job.tags << tag
}

fn (left Point) + (right Point) Point {
	return Point{left.x + right.x, left.y + right.y}
}

[inline]
fn clamp(value int, low int, high int) int {
	if value < low {
		return low
	} else if value > high {
		return high
	}
	return value
}

fn literal_gallery(name string) []string {
	double := "escaped quote: \" tab: \t unicode: \u03bb"
	single := 'interpolation $name and ${name.len}'
	raw_single := r'raw \n $name %s'
	raw_double := r"raw double $name %08x"
	c_string := c'bytes\x21'
	c_double := c"foreign\ntext"
	rune_value := `λ`
	format := 'integer=%+08d hex=%[2]x float=%.2f percent=%%'
	unknown_escape := 'oracle-only \q escape'
	multiline := 'first line café λ
second line 東京 🚀 𝌆
third line closes the V string'
	return [double, single, raw_single, raw_double, c_string, c_double,
		'${rune_value}', format, unknown_escape, multiline]
}

fn numeric_gallery() {
	mut signed := i64(-42)
	unsigned := u32(42)
	decimal := 12_345
	binary := 0b1100_0011
	octal := 0o7_5_5
	hex := 0xCA_FE
	float_value := 12.50
	exponential := 1.25E-10
	truth := bool(true)
	ptr := voidptr(0)
	signed += int(unsigned)
	assert signed != 0 || truth
	println('${decimal} ${binary} ${octal} ${hex} ${float_value} ${exponential} ${ptr}')
}

fn control_flow(mut jobs []Job) !int {
	mut total := 0
	for index, mut job in jobs {
		match job.phase {
			.queued {
				job.phase = .running
				continue
			}
			.running { total += index }
			.done { break }
		}
	}
	for total < 100 {
		total++
		if total % 9 == 0 {
			defer { println('checkpoint $total') }
		}
	}
	unsafe {
		total = clamp(total, 0, 100)
	}
	return total
}

fn concurrency(channel chan int, shared_state shared map[string]int) {
	go println('go task')
	spawn println('spawn task')
	lock shared_state {
		shared_state['count'] = 1
	}
	rlock shared_state {
		println(shared_state['count'])
	}
	select {
		value := <-channel { println(value) }
		else { println('idle') }
	}
}

fn option_and_result(input string) ?int {
	if input == '' {
		return none
	}
	return input.int()
}

fn compile_time_paths() {
	$if linux {
		println('linux')
	} $else {
		println('other')
	}
	value := sizeof(Job)
	kind := typeof(value).name
	offset := __offsetof(Job, phase)
	println('${value} ${kind} ${offset}')
}

fn main() {
	mut jobs := [new_job(UserId(7), 'compiler')]
	jobs[0].add_tag('Unicode café λ 東京 🚀 𝌆')
	result := control_flow(mut jobs) or { 0 }
	println('${identity<string>(jobs[0].display())}: ${result}')
	numeric_gallery()
	compile_time_paths()
}
