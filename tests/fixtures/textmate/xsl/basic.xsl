<?xml version="1.0" encoding="UTF-8"?>
<?xml-stylesheet type="text/xsl" href="preview.xsl"?>
<xsl:stylesheet version="1.0"
  xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
  xmlns:ex="urn:example:catalog" exclude-result-prefixes="ex">
  <xsl:output method="xml" encoding="UTF-8" indent="yes"/>
  <xsl:param name="minimum" select="10"/>
  <!-- Compact XSL coverage: café, 東京, λ, 🚀, and 𝌆. -->
  <xsl:template match="/ex:catalog">
    <report generated="true">
      <xsl:attribute name="count"><xsl:value-of select="count(ex:item)"/></xsl:attribute>
      <xsl:for-each select="ex:item[number(@price) &gt;= $minimum]">
        <entry id="{@id}">
          <xsl:value-of select="concat(ex:name, ' — 🚀')"/>
        </entry>
      </xsl:for-each>
      <note><![CDATA[Literal <markup> and astral 𝌆 remain text.]]></note>
    </report>
  </xsl:template>
</xsl:stylesheet>
