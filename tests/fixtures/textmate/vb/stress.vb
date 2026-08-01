#Const TRACE_FIXTURE = True
#If DEBUG Then
#Const BUILD_LABEL = "debug"
#Else
#Const BUILD_LABEL = "release"
#End If

Option Strict On
Option Explicit On
Option Infer On
Option Compare Text
Imports System
Imports System.Collections.Generic
Imports System.Linq
Imports System.Threading.Tasks
Imports System.Xml.Linq
Imports IO = System.IO
Imports <xmlns:ui = "urn:mark:ui">

<Assembly: CLSCompliant(True)>
Namespace TextMate.VBFixtures
    #Region "Contracts and value types"
    ''' <summary>Transforms café data while preserving 東京 and 🚀.</summary>
    Public Delegate Function Converter(Of In T, Out TResult)(value As T) As TResult
    <Flags>
    Public Enum AccessMode As Integer
        None = 0
        Read = 1
        Write = 2
        Execute = 4
        All = Read Or Write Or Execute
    End Enum
    Public Structure Coordinate
        Public ReadOnly X As Double
        Public ReadOnly Y As Double
        Public Sub New(x As Double, y As Double)
            Me.X = x
            Me.Y = y
        End Sub
        Public ReadOnly Property Length As Double
            Get
                Return Math.Sqrt(X * X + Y * Y)
            End Get
        End Property
        Public Shared Operator +(left As Coordinate, right As Coordinate) As Coordinate
            Return New Coordinate(left.X + right.X, left.Y + right.Y)
        End Operator
        Public Overrides Function ToString() As String
            Return $"({X:F2}, {Y:F2})"
        End Function
    End Structure
    Public Structure Slot(Of T As Structure)
        Public Property Value As T
        Public Property Occupied As Boolean
    End Structure
    Public Interface IRepository(Of T)
        Event Changed As EventHandler
        ReadOnly Property Count As Integer
        Default Property Item(index As Integer) As T
        Sub Add(value As T)
        Function Find(predicate As Predicate(Of T)) As T
    End Interface
    #End Region

    <Serializable>
    Public Class MemoryRepository(Of T As {Class, IComparable(Of T), New})
        Implements IRepository(Of T)
        Private ReadOnly _items As New List(Of T)()
        Private ReadOnly _gate As New Object()
        Private _name As String
        Public Event Changed As EventHandler Implements IRepository(Of T).Changed
        Public Sub New(Optional name As String = "default")
            _name = name
        End Sub
        Public Property Name As String
            Get
                Return _name
            End Get
            Private Set(value As String)
                _name = value
            End Set
        End Property
        Public ReadOnly Property Count As Integer Implements IRepository(Of T).Count
            Get
                Return _items.Count
            End Get
        End Property
        Default Public Property Item(index As Integer) As T Implements IRepository(Of T).Item
            Get
                Return _items(index)
            End Get
            Set(value As T)
                _items(index) = value
                OnChanged()
            End Set
        End Property
        Public Sub Add(value As T) Implements IRepository(Of T).Add
            If value Is Nothing Then Throw New ArgumentNullException(NameOf(value))
            SyncLock _gate
                _items.Add(value)
            End SyncLock
            OnChanged()
        End Sub
        Public Function Find(predicate As Predicate(Of T)) As T Implements IRepository(Of T).Find
            Return _items.Find(predicate)
        End Function
        Public Function ConvertAll(Of TResult)(projection As Converter(Of T, TResult)) As List(Of TResult)
            Dim results As New List(Of TResult)()
            For Each value As T In _items
                results.Add(projection(value))
            Next
            Return results
        End Function
        Public Iterator Function Values() As IEnumerable(Of T)
            For Each value As T In _items
                Yield value
            Next
        End Function
        Protected Overridable Sub OnChanged()
            RaiseEvent Changed(Me, EventArgs.Empty)
        End Sub
    End Class

    Public NotInheritable Class LiteralGallery
        Private Sub New()
        End Sub
        Public Shared Function Describe() As String
            ' BMP: naïve λ 東京; astral: 🚀 𝌆. Apostrophes stay inside comments.
            Dim signed As Integer = -1_024
            Dim hexValue As UInteger = &HCA_FEUI
            Dim octalValue As Integer = &O755
            Dim binaryValue As UShort = &B1010_0011US
            Dim longValue As Long = 9_223_372_036_854_775L
            Dim ratio As Single = 0.625F
            Dim distance As Double = 6.022E+23R
            Dim price As Decimal = 12.50D
            Dim built As Date = #12/31/2025 11:59:58 PM#
            Dim letter As Char = "λ"c
            Dim escaped As String = "She said ""VB.NET"" — 東京 🚀."
            Dim path As String = "C:\fixtures\visual-basic"
            Dim message As String = $"{letter}: {price:C2} at {built:O}"
            Return String.Join("|", signed, hexValue, octalValue, binaryValue,
                               longValue, ratio, distance, path, escaped, message)
        End Function
    End Class
    Public Module QueryAndXmlSamples
        Private ReadOnly Gate As New Object()
        Public Event Reported(message As String)
        Public Function BuildDocument(names As IEnumerable(Of String)) As XElement
            Dim filtered = From name In names
                           Let trimmed = name.Trim()
                           Where trimmed.Length > 0 AndAlso Not trimmed.StartsWith("_")
                           Order By trimmed.Length Descending, trimmed Ascending
                           Select Label = trimmed, Size = trimmed.Length
            Dim document =
                <catalog generated=<%= Date.Now %>>
                    <!-- fully closed XML literal: café, 東京, and 🚀 -->
                    <ui:item id="first"><![CDATA[<escaped> & ready 𝌆]]></ui:item>
                    <items>
                        <%= From entry In filtered
                            Select <item size=<%= entry.Size %>><name><%= entry.Label %></name></item> %>
                    </items>
                </catalog>
            Dim firstName As String = document.<items>.<item>.<name>.FirstOrDefault()?.Value
            Dim namespacedId As String = CStr(document.<ui:item>.First().@id)
            document.SetAttributeValue("summary", $"{firstName}:{namespacedId}")
            Return document
        End Function
        Public Sub ExerciseControlFlow(values As IEnumerable(Of Integer), candidate As Object)
            Dim score As Integer = If(candidate Is Nothing, -1, 0)
            Dim text As String = TryCast(candidate, String)
            If text IsNot Nothing AndAlso text Like "[A-Z]*" Then
                score += text.Length
            ElseIf TypeOf candidate Is Integer OrElse candidate Is Nothing Then
                score = CInt(If(candidate, 0))
            Else
                score = -2
            End If
            Select Case score
                Case Is < 0
                    RaiseEvent Reported("negative")
                Case 0 To 9
                    RaiseEvent Reported("small")
                Case 10, 20, 30
                    RaiseEvent Reported("round")
                Case Else
                    RaiseEvent Reported("large")
            End Select
            For index As Integer = 0 To 5 Step 1
                If index = 1 Then Continue For
                score += index
                If score > 100 Then Exit For
            Next index
            For Each number As Integer In values
                score = (score Xor number) And &HFF
            Next number
            Do While score > 64
                score >>= 1
            Loop
            Do
                score += 1
            Loop Until score Mod 3 = 0
            While score < 12
                score += 2
            End While
            With score
                .ToString()
                .CompareTo(10)
            End With
        End Sub
        Public Sub ResourceAndErrorSample(fileName As String)
            Try
                Using stream As New IO.MemoryStream(), writer As New IO.StreamWriter(stream)
                    writer.WriteLine("payload — λ 🚀")
                    writer.Flush()
                    If fileName = String.Empty Then
                        Throw New InvalidOperationException("A file name is required.")
                    End If
                End Using
            Catch ex As InvalidOperationException When ex.Message.Length > 0
                RaiseEvent Reported(ex.Message)
            Catch ex As Exception
                RaiseEvent Reported(ex.GetType().Name)
            Finally
                SyncLock Gate
                    RaiseEvent Reported("finished")
                End SyncLock
            End Try
        End Sub
        Public Async Function DelayAsync(milliseconds As Integer) As Task(Of Integer)
            Await Task.Delay(milliseconds).ConfigureAwait(False)
            Return milliseconds
        End Function
        Public Sub Adjust(ByRef total As Integer, ParamArray deltas() As Integer)
            For Each delta As Integer In deltas
                total += delta
            Next
        End Sub
    End Module
End Namespace
