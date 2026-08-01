#!/usr/bin/env groovy
package fixtures.textmate.groovy

import java.time.Instant
import static java.lang.Math.max

/** Small oracle fixture: BMP λ and astral 🚀. */
@Deprecated(since = 'fixture')
class Greeting {
    static final String CITY = '東京'

    String render(String name = 'Ada') {
        def details = [active: true, count: 2, missing: null]
        def message = """Hello ${name}, λ from ${CITY}!
Launch ${details.count}: 🚀"""
        return message
    }
}

assert new Greeting().render().contains('🚀') : 'unicode survives'
