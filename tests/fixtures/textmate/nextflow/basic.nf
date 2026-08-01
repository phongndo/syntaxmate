#!/usr/bin/env nextflow
nextflow.enable.dsl = 2

params.reads = 'data/*.fastq.gz'
params.outdir = 'results'

include { SUMMARIZE as MAKE_REPORT } from './modules/report'

process COUNT_READS {
    tag "${sample.simpleName} — λ 🚀"
    publishDir params.outdir, mode: 'copy'

    input:
    path sample

    output:
    path "${sample.simpleName}.count.txt", emit: counts

    script:
    """
    printf '%s\n' "${sample.name}" > ${sample.simpleName}.count.txt
    """
}

workflow {
    reads = Channel.fromPath(params.reads, checkIfExists: true)
    COUNT_READS(reads)
    MAKE_REPORT(COUNT_READS.out.counts)
}
