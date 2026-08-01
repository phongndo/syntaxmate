#!/usr/bin/env nextflow
nextflow.enable.dsl = 2

/*
 * Grammar-oriented DSL2 pipeline fixture.
 * BMP glyphs: λ, β, 東京. Astral glyphs: 🚀, 🧬, 𝌆.
 * Every spanning comment, string, closure, and block is closed.
 */

include { VALIDATE_SAMPLES; TRIM_READS as TRIM } from './modules/prepare'
include { ALIGN_READS; SORT_BAM } from './modules/alignment'
include { CALL_VARIANTS as CALL; INDEX_VCF } from './modules/variants'
include { MULTIQC as BUILD_REPORT } from './modules/report'

params {
    input: Path
    reference: Path
    outdir: Path = 'results'
    min_reads: Integer = 1000
    max_cpus: Integer = 8
    publish_mode: String = 'copy'
    greeting: String = '東京 λ 🚀'
}

record SampleSheetRow {
    String id
    Path reads
    String group
    boolean enabled
}

record AlignmentResult {
    String id
    Path bam
    Path bai
    Map metadata
}

enum PipelineStage {
    DISCOVER,
    PREPARE,
    ALIGN,
    CALL,
    REPORT
}

enum ExitCode {
    SUCCESS(0),
    INVALID_INPUT(2),
    TOOL_FAILURE(10)
}

def safe_label(String value, int limit = 32) {
    def cleaned = value.trim().replaceAll(/[^A-Za-z0-9_.-]+/, '_')
    return cleaned.take(limit) ?: 'sample'
}

def resource_scale(int attempt, int ceiling = 8) {
    return Math.min(attempt * 2, ceiling)
}

process INSPECT_REFERENCE {
    tag "reference:${reference.simpleName}"
    label 'small'
    cpus 1
    memory '1 GB'
    publishDir "${params.outdir}/reference", mode: params.publish_mode
    input:
    path reference
    output:
    tuple path(reference), path("${reference}.fai"), emit: indexed_reference
    path 'reference-metadata.json', emit: metadata
    script:
    def banner = "λ-index-${reference.simpleName}"
    """
    samtools faidx ${reference}
    printf '{"banner":"%s","symbol":"🧬"}\n' '${banner}' > reference-metadata.json
    """
}

process NORMALIZE_READS {
    tag { "${meta.id}:${task.attempt}" }
    label 'medium'
    cpus { resource_scale(task.attempt, params.max_cpus) }
    memory { 2.GB * task.attempt }
    time { 30.minutes * task.attempt }
    errorStrategy { task.exitStatus in 137..140 ? 'retry' : 'terminate' }
    maxRetries 2
    publishDir "${params.outdir}/clean", mode: params.publish_mode, overwrite: false
    input:
    tuple val(meta), path(reads)
    env PIPELINE_TOKEN
    output:
    tuple val(meta), path("${meta.id}.clean.fastq.gz"), emit: cleaned
    tuple val(meta.id), env(PIPELINE_TOKEN), emit: audit
    path 'versions-normalize.yml', emit: versions
    when:
    meta.enabled && reads.size() > 0
    script:
    def prefix = safe_label(meta.id)
    def note = "${params.greeting} / ${meta.group}"
    """
    gzip -cd ${reads} | awk 'NF' | gzip > ${prefix}.clean.fastq.gz
    printf 'normalize: 1.0\nnote: "%s"\n' '${note}' > versions-normalize.yml
    """
}

process CALCULATE_STATS {
    tag "$meta.id"
    label 'small'
    cache 'lenient'
    input:
    tuple val(meta), path(cleaned_reads)
    output:
    tuple val(meta), path("${meta.id}.stats.tsv"), emit: table
    stdout emit: command_log

    shell:
    '''
    printf 'sample\treads\n'
    printf '!{meta.id}\t%s\n' "$(gzip -cd !{cleaned_reads} | wc -l)" \
        > !{meta.id}.stats.tsv
    '''
}

process MERGE_SUMMARIES {
    tag "batch-${batch_id}"
    label 'medium'
    storeDir "${params.outdir}/cache"
    input:
    val batch_id
    path tables
    output:
    tuple val(batch_id), path("summary-${batch_id}.tsv"), emit: summary
    path 'merge-command.txt', emit: provenance

    script:
    def inputs = tables.collect { it.name }.join(' ')
    """
    cat ${inputs} > summary-${batch_id}.tsv
    echo "cat ${inputs}" > merge-command.txt
    """
}

process WRITE_MANIFEST {
    tag 'manifest'

    input:
    path summaries

    output:
    path 'manifest.json', emit: manifest

    exec:
    def rows = summaries.collect { file ->
        [name: file.name, bytes: file.size(), stage: PipelineStage.REPORT]
    }
    def payload = [created: new Date(), rows: rows, glyph: '𝌆']
    file('manifest.json').text = groovy.json.JsonOutput.prettyPrint(
        groovy.json.JsonOutput.toJson(payload)
    )
}

workflow PREPARE_INPUTS {
    take:
    sample_rows

    main:
    enabled_rows = sample_rows.filter { row -> row.enabled }
    read_pairs = enabled_rows.map { row ->
        def meta = [id: row.id, group: row.group, enabled: row.enabled]
        tuple(meta, row.reads)
    }
    NORMALIZE_READS(read_pairs)
    CALCULATE_STATS(NORMALIZE_READS.out.cleaned)

    emit:
    clean_reads = NORMALIZE_READS.out.cleaned
    stats = CALCULATE_STATS.out.table
    logs = CALCULATE_STATS.out.command_log
}

workflow ANALYZE_COHORT {
    take:
    prepared_reads
    reference_index

    main:
    ALIGN_READS(prepared_reads, reference_index)
    SORT_BAM(ALIGN_READS.out.bam)
    CALL(SORT_BAM.out.sorted, reference_index)
    INDEX_VCF(CALL.out.vcf)

    emit:
    bam = SORT_BAM.out.sorted
    variants = INDEX_VCF.out.indexed
}

workflow {
    main:
    samples = channel
        .fromPath(params.input, checkIfExists: true)
        .splitCsv(header: true)
        .map { row ->
            new SampleSheetRow(
                row.sample as String,
                file(row.reads),
                (row.group ?: 'default') as String,
                row.enabled == null || row.enabled.toBoolean()
            )
        }

    reference_ch = Channel.value(params.reference)
    INSPECT_REFERENCE(reference_ch)
    VALIDATE_SAMPLES(samples)
    PREPARE_INPUTS(VALIDATE_SAMPLES.out.valid)
    ANALYZE_COHORT(PREPARE_INPUTS.out.clean_reads, INSPECT_REFERENCE.out.indexed_reference)

    grouped_stats = PREPARE_INPUTS.out.stats
        .map { meta, table -> tuple(meta.group, table) }
        .groupTuple()
    MERGE_SUMMARIES(grouped_stats)
    WRITE_MANIFEST(MERGE_SUMMARIES.out.summary.map { batch, table -> table }.collect())
    BUILD_REPORT(
        PREPARE_INPUTS.out.stats.mix(ANALYZE_COHORT.out.variants),
        Channel.value("cohort-β-🚀")
    )

    emit:
    cleaned = PREPARE_INPUTS.out.clean_reads
    alignments = ANALYZE_COHORT.out.bam
    variants = ANALYZE_COHORT.out.variants
    report = BUILD_REPORT.out.report
    manifest = WRITE_MANIFEST.out.manifest

    publish:
    report = { path "reports", mode: params.publish_mode }
    manifest = { path "metadata", mode: 'copy', enabled: true }
}

output {
    reports {
        path 'reports/*.html'
        index 'index.html'
    }
    metadata {
        path 'metadata/manifest.json'
    }
}
