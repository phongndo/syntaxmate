#!/usr/bin/env nextflow
/* Embedded Groovy basics: BMP λ 東京, astral 🚀 𝌆. */
def sample = [id: '東京', active: true, missing: null]
List<Integer> counts = [1, 2, 3]
String greeting = "Hello $sample.id — λ 🚀"
def detail = "size=${counts.size()} nested=${[ok: { -> '𝌆' }].ok()}"
def literal = '''single
quoted block'''
def banner = """double
interpolation: ${sample.id}
closed"""
def word = /read_[0-9]+\.fq/
def compiled = ~"(?i)${sample.id}"
def scale = { int value, int step = 2 -> value * step }
def result = sample?.id ?: 'unknown'
assert result ==~ compiled || scale(counts[0]) == 2 : greeting
