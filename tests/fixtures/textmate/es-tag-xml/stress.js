const hostLabel = "XML fixtures café 東京 🚀 𝌆";
const hostVersion = 3;
const collect = (...documents) => documents.map((text) => text.length);

const atomFeed = xml`<?xml version="1.0" encoding="UTF-8"?>
  <feed xmlns="http://www.w3.org/2005/Atom" xml:lang="en">
    <id>urn:uuid:2f1d7d70-8f31-4dc9-a25b-9caaf00b1234</id>
    <title>Release café 東京 🚀 𝌆</title>
    <updated>2025-06-01T12:30:00Z</updated>
    <author>
      <name>Example Team</name>
      <email>team@example.test</email>
    </author>
    <!-- Entries are complete and namespace-aware. -->
    <entry>
      <id>urn:example:entry:1</id>
      <title type="text">First &amp; foremost</title>
      <link href="https://example.test/posts/1?from=atom&amp;lang=en" />
      <summary type="html">&lt;p&gt;Escaped markup&lt;/p&gt;</summary>
      <category term="news" scheme="urn:example:categories" />
    </entry>
    <entry>
      <id>urn:example:entry:2</id>
      <title>Second entry</title>
      <content type="text">Five is &lt; six and seven is &gt; six.</content>
    </entry>
  </feed>
`;
const afterAtom = atomFeed.length;

const catalog = inline-xml`<?xml version="1.0"?>
  <catalog xmlns="urn:example:catalog" xmlns:m="urn:example:meta">
    <m:metadata m:version="3">
      <m:owner>Research &amp; Development</m:owner>
      <m:note><![CDATA[Literal <angle> text & unescaped data; café 東京 🚀 𝌆]]></m:note>
    </m:metadata>
    <product id="p-100" available="true">
      <name>Café grinder</name>
      <price currency="EUR">89.50</price>
      <dimensions unit="cm">
        <width>12</width>
        <height>24</height>
        <depth>10</depth>
      </dimensions>
    </product>
    <product id="p-200" available="false">
      <name>東京 dripper</name>
      <price currency="JPY">3200</price>
      <tags>
        <tag>ceramic</tag>
        <tag>limited</tag>
      </tags>
    </product>
  </catalog>
`;
const afterCatalog = catalog.startsWith("<?xml");

const vectorIcon = /* svg */ `
  <svg xmlns="http://www.w3.org/2000/svg"
       xmlns:xlink="http://www.w3.org/1999/xlink"
       viewBox="0 0 120 120"
       role="img"
       aria-labelledby="icon-title icon-desc">
    <title id="icon-title">Rocket icon 🚀</title>
    <desc id="icon-desc">A complete layered vector image</desc>
    <defs>
      <linearGradient id="flame" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0%" stop-color="#ffcc00" />
        <stop offset="100%" stop-color="#ff3300" />
      </linearGradient>
      <clipPath id="window-clip">
        <circle cx="60" cy="42" r="12" />
      </clipPath>
    </defs>
    <!-- XML-style self-closing elements are intentional. -->
    <g id="rocket" transform="translate(0 2)">
      <path d="M60 8 C82 28 84 72 60 96 C36 72 38 28 60 8 Z" fill="#e8eef7" />
      <circle cx="60" cy="42" r="11" fill="#336699" clip-path="url(#window-clip)" />
      <path d="M48 88 L36 108 L52 101 Z" fill="#667788" />
      <path d="M72 88 L84 108 L68 101 Z" fill="#667788" />
      <path d="M53 96 L60 116 L67 96 Z" fill="url(#flame)" />
    </g>
  </svg>
`;
const afterVector = vectorIcon.includes("</svg>");

const configuration = /* xml */ `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
  <configuration environment="test">
    <database host="db.example.test" port="5432">
      <pool minimum="2" maximum="12" timeout="30" />
    </database>
    <features>
      <feature name="search" enabled="true" />
      <feature name="preview" enabled="false" />
      <feature name="unicode" enabled="true">
        <sample>café 東京 🚀 𝌆</sample>
      </feature>
    </features>
    <logging level="info">
      <sink type="console" format="json" />
      <sink type="file" path="logs/app.log" rotate="daily" />
    </logging>
  </configuration>
`;
const afterConfiguration = configuration.trim().length;

const manifest = /* inline-xml */ `
  <manifest xmlns="urn:example:manifest" revision="7">
    <files>
      <file path="index.html" media-type="text/html" size="1024" />
      <file path="styles/main.css" media-type="text/css" size="2048" />
      <file path="scripts/app.js" media-type="text/javascript" size="4096" />
    </files>
    <checksums algorithm="sha256">
      <checksum path="index.html">abc123</checksum>
      <checksum path="styles/main.css">def456</checksum>
      <checksum path="scripts/app.js">789abc</checksum>
    </checksums>
    <signature valid="true">example-signature</signature>
  </manifest>
`;
const afterManifest = manifest.match(/<file /g).length;

const commentSelected =
  // xml
  `<message xmlns="urn:example:message" priority="high">
    <from>system@example.test</from>
    <to>reader@example.test</to>
    <subject>Comment marker form</subject>
    <body>Quotes: &quot;hello&quot;; apostrophe: &apos;yes&apos;.</body>
  </message>`;
const afterCommentSelected = commentSelected.length;

const svgCommentSelected =
  // svg
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40">
    <rect x="2" y="2" width="36" height="36" rx="6" fill="none" stroke="currentColor" />
    <path d="M10 20 L17 27 L31 12" fill="none" stroke="currentColor" />
  </svg>`;
const afterSvgComment = svgCommentSelected.trim();

const inlineCommentSelected =
  // inline-xml
  `<response xmlns="urn:example:api" status="ok">
    <!-- A multiline XML comment
         contains café 東京 🚀 𝌆
         and closes before the document does. -->
    <request-id>req-12345</request-id>
    <result count="2">
      <item key="alpha" />
      <item key="beta" />
    </result>
  </response>`;
const afterInlineComment = inlineCommentSelected.includes("request-id");

const documentWithDoctype = xml`<?xml version="1.0"?>
  <!DOCTYPE greeting [
    <!ELEMENT greeting (#PCDATA)>
    <!ATTLIST greeting language CDATA #REQUIRED>
  ]>
  <greeting language="fr">Bonjour, café &amp; thé.</greeting>
`;
const afterDoctype = documentWithDoctype.length > 0;

const processing = inline-xml`
  <?xml-stylesheet type="text/xsl" href="theme.xsl"?>
  <report generated="2025-06-01T12:30:00Z">
    <section name="summary">
      <metric name="requests" value="1200" unit="count" />
      <metric name="latency" value="24.5" unit="ms" />
    </section>
    <section name="notes">
      <note xml:space="preserve">  spacing is retained  </note>
    </section>
  </report>
`;
const afterProcessing = processing.slice(-10);

function summarizeXml() {
  const lengths = collect(atomFeed, catalog, vectorIcon, configuration, manifest);
  return {
    hostLabel,
    hostVersion,
    lengths,
    afterAtom,
    afterCatalog,
    afterVector,
    afterConfiguration,
    afterManifest,
    afterCommentSelected,
    afterSvgComment,
    afterInlineComment,
    afterDoctype,
    afterProcessing,
  };
}

export { atomFeed, catalog, vectorIcon, configuration, manifest };
export default summarizeXml;
