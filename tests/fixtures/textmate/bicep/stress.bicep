targetScope = 'subscription'

// Grammar-focused Bicep fixture: café, Ελληνικά, 日本語, 🚀, and 𝌆.
/* The declarations below exercise comments, decorators, literals, nested
   expressions, comprehensions, lambdas, resources, modules, and outputs. */
metadata fixture = {
  purpose: 'TextMate Tier B promotion'
  unicode: 'Zażółć gęślą jaźń — 世界 — 🚀 𝌆'
}

param prefix string = 'mark'

param environment string = 'dev'

param regions array = [
  'eastus'
  'westeurope'
  'japaneast'
]

param extraTags object = {}

param enableDiagnostics bool = true

param contactEmail string

type EnvironmentName = 'dev' | 'test' | 'prod'

type ServiceSettings = {
  displayName: string
  environment: EnvironmentName
  replicas: int
  features: string[]
}

var settings = {
  displayName: 'Syntax Café'
  environment: environment
  replicas: environment == 'prod' ? 3 : 1
  features: [
    'metrics'
    'alerts'
    enableDiagnostics ? 'diagnostics' : 'minimal'
  ]
}

var escaped = 'quote=\' slash=\\ newline=\n tab=\t rocket=\u{1F680} literal=\${prefix}'
var plainMultiline = '''
Line one keeps apostrophes like it's ordinary text.
Line two contains café, 日本語, 🚀, and 𝌆.
'''
var interpolatedMultiline = $'''
Service ${settings.displayName}
Environment ${environment}
'''
var doubledInterpolation = $$'''
Literal ${prefix}; active $${toUpper(prefix)}.
'''

var normalizedPrefix = toLower(replace(prefix, '_', '-'))
var compactPrefix = take(replace(normalizedPrefix, '-', ''), 10)
var uniqueSeed = uniqueString(subscription().id, environment)
var deploymentStamp = '${compactPrefix}-${environment}-${take(uniqueSeed, 6)}'
var optionalOwner = contains(extraTags, 'owner') ? extraTags.owner : null

var commonTags = union(extraTags, {
  application: 'mark-syntax'
  environment: environment
  owner: optionalOwner ?? 'platform'
  locale: '日本語'
  symbol: '🚀'
})

var regionObjects = [for (region, index) in regions: {
  name: region
  ordinal: index + 1
  primary: index == 0
  code: toUpper(take(region, 3))
}]

var enabledRegions = filter(regionObjects, item => item.name != 'antarctica')
var regionNames = map(enabledRegions, item => item.name)
var sortedNames = sort(regionNames, (left, right) => left < right)
var nameLengthTotal = reduce(regionNames, 0, (total, item) => total + length(item))
var allRegionsNamed = every(regionObjects, item => !empty(item.name))
var hasJapan = some(regionNames, item => contains(item, 'japan'))

var regionMap = {for item in regionObjects: item.name: {
  ordinal: item.ordinal
  label: '${item.code}-${item.ordinal}'
}}

var matrix = [
  [1, 2, 3]
  [4, 5, 6]
  [7, 8, 9]
]
var selectedNumber = matrix[1][2]
var arithmetic = ((selectedNumber * 4) + 8) / 2
var comparisons = arithmetic >= 10 && arithmetic <= 100 || false
var fallbackRegion = regions[0] ?? 'eastus'

assert prefixLength = length(prefix) >= 3
assert regionsPresent = length(regions) > 0

#disable-next-line no-hardcoded-env-urls
var healthEndpoint = 'https://${deploymentStamp}.example.test/health'

resource resourceGroup 'Microsoft.Resources/resourceGroups@2022-09-01' = [for item in regionObjects: {
  name: 'rg-${deploymentStamp}-${item.code}'
  location: item.name
  tags: union(commonTags, {
    region: item.name
    ordinal: string(item.ordinal)
  })
}]

resource existingGroup 'Microsoft.Resources/resourceGroups@2022-09-01' existing = {
  name: resourceGroup[0].name
}

module regional './modules/regional.bicep' = [for (item, index) in regionObjects: if (item.primary || environment != 'dev') {
  name: 'regional-${item.code}-${index}'
  scope: resourceGroup[index]
  params: {
    location: item.name
    serviceName: deploymentStamp
    settings: settings
    tags: commonTags
    diagnostics: enableDiagnostics
  }
  dependsOn: [
    existingGroup
  ]
}]

resource policy 'Microsoft.Authorization/policyAssignments@2022-06-01' = if (environment == 'prod') {
  name: 'require-mark-tag'
  scope: resourceGroup[0]
  properties: {
    displayName: 'Require the mark tag — conformité'
    description: plainMultiline
    policyDefinitionId: subscriptionResourceId(
      'Microsoft.Authorization/policyDefinitions'
      'require-tag'
    )
    parameters: {
      tagName: {
        value: 'application'
      }
    }
    enforcementMode: 'Default'
  }
}

var summary = {
  name: deploymentStamp
  location: fallbackRegion
  regionCount: length(regionObjects)
  characterCount: nameLengthTotal
  selectedNumber: selectedNumber
  healthy: comparisons && allRegionsNamed
  japanIncluded: hasJapan
  endpoint: healthEndpoint
  labels: regionMap
  message: interpolatedMultiline
  escaped: escaped
}

output deploymentName string = deploymentStamp

output resourceGroupIds array = [for group in resourceGroup: group.id]

output deploymentSummary object = summary

output regionalOutputs array = [for item in regional: item.outputs]
