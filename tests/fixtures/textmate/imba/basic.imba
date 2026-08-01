#!/usr/bin/env imba
# Compact Imba coverage: Երևան, λ, 東京, and 🚀.
import { join } from 'path'

const title = "Night queue 🚀"
let count = 0x2A
let flags = [true, false, yes, no, null, undefined]
let record = {name: 'Անի', city: "東京", score: 3.5e2}
let matcher = /^(?<word>[A-Z]+)\s+[\u0400-\u04ff]+$/giu

class Greeter
	def constructor prefix = 'Hello'
		@prefix = prefix
	def render name
		"{@prefix}, {name}!"

tag app-root
	<self.main#root data-count=count @click.prevent={count++}>
		<header.title> title
		<p.note[ color: rebeccapurple ]> "{record.name} — {record.city}"

css .main:hover > .note
	color: #2a7fff
	padding: calc(1rem + 2px)
