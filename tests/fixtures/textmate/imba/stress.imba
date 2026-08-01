#!/usr/bin/env imba
/// <reference types="node" />
# Observatory dashboard fixture: Երևան, naïve café, Διάστημα, 東京, 🔭, and 𐍈.
# Realistic declarations exercise Imba, embedded styles, tags, regex, and docs.
import { readFile, writeFile } from 'fs/promises'
import type { PathLike } from 'fs'
import QueueStore from './queue-store'
###
 * Normalize one observing request for the night queue.
 * @author Ada Example <ada@example.test>
 * @template T
 * @param {Object} row source record
 * @returns {Object} a safe queue entry
 * @see https://example.test/queue
 * Related helper: {@link QueueStore|persistent queue}.
###
def normalize-row row
	let name = row.name ?? 'unnamed'
	let exposure = Math.max(1, Math.min(900, Number(row.exposure or 30)))
	let repeats = Math.max(1, Number(row.repeats or 1))
	{name: name, ra: Number(row.ra), dec: Number(row.dec), exposure: exposure, repeats: repeats}

# @internal parser details stay private
def parse-coordinate value
	let clean = String(value).trim!
	if clean == ''
		throw new Error('empty coordinate')
	let parsed = Number(clean)
	if Number.isNaN(parsed)
		throw new Error("invalid coordinate: {clean}")
	parsed
###
This legacy block comment spans lines and carries Unicode 東京 🔭.
It also validates that ordinary code resumes after the closing marker.
###
const APP_NAME = 'Night Queue'
const SIDEREAL_DAY = 23.9344696
const HEX_MASK = 0xFF_A0n
const BINARY_FLAGS = 0b1010_0011n
const OCTAL_MODE = 0o640n
const SCIENCE = 6.022e23
const LEADING = .625
const TRAILING = 12.
const CSS_TIME = 250ms
const CSS_RATIO = 62.5%
const NOT_A_NUMBER = NaN
const FOREVER = Infinity
let optional = undefined
let empty = null
let enabled = true
let disabled = false
let affirmative = yes
let negative = no

class Target<T>
	prop name
	prop ra
	prop dec
	attr selected
	def constructor data
		@name = data.name
		@ra = data.ra
		@dec = data.dec
		@selected = false
	def duration exposure, repeats = 1
		exposure * repeats
	get label
		"{@name} ({@ra}, {@dec})"
	set active value
		@selected = Boolean(value)
	private def secret-code
		#internal-value
	static def from row
		new Target(normalize-row(row))

interface Serializable
	def serialize

mixin Timestamped
	def timestamp
		Date.now!

struct Point
	let x
	let y

tag queue-card
	prop target
	def render
		<self.card#queue-$target.name.selected[data-kind='target' i] title="Target {$target.name}" @click.prevent.stop={toggle!} /** @param {Target} target */>
			<header.card__header>
				<h2.title> $target.name
				<span.badge[ color: currentColor; opacity: 75% ]> "{$target.repeats}×"
			<section.body>
				<slot>
				<p.coords> "{$target.ra}, {$target.dec}"
			<footer%muted $footer-ref>
				<button.primary @keydown.enter={toggle!} disabled=!$target> 'Toggle'
	def toggle
		$target.selected = !$target.selected

tag queue-app
	def render
		<self.shell.theme-dark>
			<nav[ display: flex; gap: 1rem ]>
				<a.item href='/'> 'Queue'
				<a.item href='/about'> 'About 🚀'
			<main>
				<queue-card(target) @select={save-target(target)}>
				<p.empty> "No requests for 東京"

global scoped css .shell, .theme-dark
	--accent: #2a7fff
	color: rebeccapurple
	background: linear-gradient(135deg, navy 0%, rgba(10, 20, 40, .9) 100%)
	width: calc(100% - 2rem)
	min-height: 100vh
	transition: opacity 250ms cubic-bezier(.2, .8, .2, 1)
	transform: translateX(0px) rotate(1deg)
	clip-path: polygon(0 0, 100% 0, 100% 100%, 0 100%)
	filter: drop-shadow(0 2px 8px rgba(0,0,0,.25))
	font-size: md
	content: "Unicode \\1F680"
	&:hover > .card::before
		color: currentColor !important
		opacity: calc(1 - .08)
	[data-kind^='tar' i] + .card ~ .empty
		border-color: hsl(210, 80%, 50%)
		outline: 1px solid transparent

%muted:hover
	color: gray
	opacity: 65%
	&::selection
		background: gold

export css .card
	display: grid
	grid-template-columns: minmax(12rem, 1fr) fit-content(20rem)
	padding: 1rem 2rem
	margin: 0 auto
	border-radius: 8px
	background: radial-gradient(circle at top, aliceblue, transparent)
	@sm-color: tomato
	.visible: block
	%busy: opacity(.7)

def total-duration targets
	let total = 0
	for own target, index in targets
		total += target.exposure * target.repeats
		continue if target.name == 'maintenance'
		break if index >= 100
	total

def group-by-hemisphere targets
	let grouped = {north: [], south: []}
	for target of targets
		let key = target.dec < 0 ? 'south' : 'north'
		grouped[key].push(target)
	grouped

def visible target, latitude
	target.dec >= latitude - 90 and target.dec <= latitude + 90 and target.name isnt 'maintenance'

def status-bits ready, tracking, guiding
	let bits = 0
	bits |= 0b0001 if ready
	bits ^= 0b0010 if tracking
	bits &= 0xFF if guiding
	(bits << 2) | (bits >> 1)

def classify sample
	switch sample.kind
		when 'science' then sample.signal / sample.noise
		when 'flat' then 1
		else 0

def retry operation, attempts = 3
	let last-error = null
	for attempt in [0, 1, 2]
		try
			return await operation!
		catch error
			last-error = error
			console.warn(`attempt {attempt + 1}: {error.message}`)
		finally
			console.debug('attempt complete')
	throw last-error

def find-label text
	let matcher = /^(?<prefix>[A-Z]{2,4})[- ](?=\d)([0-9]{1,4})\s+[\u0400-\u04FF]+$/giu
	let fallback = /(?:東京|Երևան|𐍈)|[^\s]+/uy
	matcher.exec(text) ?? fallback.exec(text)

def access-demo payload
	let direct = payload.owner.name
	let optional = payload..observer..address..city
	let positional = $1
	let internal = $queue-store
	let symbol = @@refresh-now
	let private-name = ##cache-entry
	let state = :ready
	[direct, optional, positional, internal, symbol, private-name, state]

def operators a, b
	let spread = [...a, ...b]
	let compared = a === b or a !== b
	let relation = a <= b and a <> null
	let arithmetic = (a + b) * 2 / 4 % 3
	a++
	b--
	{spread: spread, compared: compared, relation: relation, arithmetic: arithmetic}

const multiline = '''Queue report
Երևան and 東京 remain visible.
Astral marker: 𐍈 🔭
'''
const escaped = "line one\nline two\t\u{1F680}"
const template = `Queue {APP_NAME}: {multiline}`
const records = [
	{name: 'M42', ra: 83.822, dec: -5.391, exposure: 45s, repeats: 3},
	{name: 'Արագած', ra: 12.5, dec: 40.5, exposure: 60s, repeats: 2},
	{name: '東京 🚀', ra: 120, dec: 35, exposure: 10s, repeats: 1},
	{name: '𐍈 field', ra: 240, dec: -20, exposure: 90s, repeats: 4}
]

@memoized
def build-report rows
	rows.map do(row)
		let target = Target.from(row)
		{name: target.name, duration: target.duration(row.exposure, row.repeats)}

### @ts
interface OracleMeta { stoppedEarly: boolean; lineCount: number }
const oracle: OracleMeta = { stoppedEarly: false, lineCount: 0 }
###

export default {APP_NAME, records, build-report, total-duration}
