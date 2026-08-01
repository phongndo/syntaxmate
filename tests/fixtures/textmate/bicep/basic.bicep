targetScope = 'resourceGroup'

// Compact Bicep coverage: café, 日本語, and astral 🚀 𝌆.
param location string = resourceGroup().location

param environment string = 'dev'

var serviceName = toLower('mark-${environment}')
var tags = {
  environment: environment
  greeting: 'Olá, 世界'
  enabled: true
}

resource storage 'Microsoft.Storage/storageAccounts@2023-05-01' = {
  name: take(replace('${serviceName}🚀', '-', ''), 24)
  location: location
  kind: 'StorageV2'
  sku: { name: 'Standard_LRS' }
  tags: tags
}

output storageId string = storage.id
