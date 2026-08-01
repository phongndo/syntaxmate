Option Strict On
Option Explicit On

Imports System

Namespace TextMate.VBFixtures
    ' Basic Unicode comment: café, 東京, 🚀, and 𝌆.
    Public Module Basic
        Public Sub Main()
            Dim answer As Integer = 42
            Dim greeting As String = "Hello, λ and 🚀!"
            Dim xml = <message language="日本語"><![CDATA[rockets <launch> 🚀]]></message>

            If answer Mod 2 = 0 AndAlso greeting.Length > 0 Then
                Console.WriteLine("{0}: {1} — {2}", answer, greeting, xml.Value)
            Else
                Console.WriteLine("Nothing to report.")
            End If
        End Sub
    End Module
End Namespace
