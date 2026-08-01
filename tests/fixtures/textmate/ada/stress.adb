limited with Ada.Containers;
with Ada.Exceptions;
with Ada.Text_IO;
use Ada.Text_IO;
use type Ada.Containers.Count_Type;

pragma Assertion_Policy (Check);
-- Ada TextMate stress fixture: café λ 東京 🚀 𝌆
-- @purpose Exercise declarations, expressions, and nested lexical states.
-- ---------------------------- Types ---------------------------- --

procedure Syntax_Atlas is
   Max_Items : constant := 12;
   Hex_Value : constant := 16#CA_FE#;
   Real_Value : constant Long_Float := 6.022_140_76E+23;
   Tiny_Value : constant Long_Float := 2.5E-4;
   Unicode_Text : constant String := "café λ 東京 🚀 𝌆";

   type State is (Idle, Loading, Ready, Failed);
   type Permissions is mod 2 ** 8;
   subtype Item_Index is Positive range 1 .. Max_Items;
   type Score_Array is array (Item_Index range <>) of Integer;
   type Score_Access is access all Score_Array;

   type Payload (Kind : State := Idle) is record
      Name : String (1 .. 8) := (others => ' ');
      case Kind is
         when Idle | Loading =>
            Progress : Natural := 0;
         when Ready =>
            Result : Integer := 16#2A#;
         when Failed =>
            Code : Integer := -1;
      end case;
   end record;

   type Tagged_Item is tagged record
      Id : Natural := 0;
      Enabled : Boolean := True;
   end record;

   type Item_Ref is not null access all Tagged_Item;
   Empty_Scores : constant Score_Array := (others => 0);
   Scores : Score_Array (1 .. Max_Items) :=
     (1 => 5, 2 => 8, 3 | 4 => 13, others => 0);
   Current : aliased Tagged_Item := (Id => 7, Enabled => True);
   Current_Ref : Item_Ref := Current'Access;
   Mask : Permissions := 2#1010_0110#;
   Problem : exception;

   function Clamp
     (Value : Integer;
      Low   : Integer := 0;
      High  : Integer := 100) return Integer
   with Pre => Low <= High
   is
   begin
      if Value < Low then
         return Low;
      elsif Value > High then
         return High;
      else
         return Value;
      end if;
   end Clamp;

   function "+" (Left, Right : State) return State is
   begin
      if Left = Failed or else Right = Failed then
         return Failed;
      end if;
      return State'Max (Left, Right);
   end "+";

   procedure Report
     (Label : String;
      Value : Integer;
      Loud  : Boolean := False)
   is
      Prefix : constant String := (if Loud then "! " else "- ");
   begin
      Put_Line (Prefix & Label & Integer'Image (Value));
   end Report;

   package Counters is
      type Counter is private;
      procedure Increment (Value : in out Counter);
      function Image (Value : Counter) return String;
   private
      type Counter is new Natural;
   end Counters;

   package body Counters is
      procedure Increment (Value : in out Counter) is
      begin
         Value := Value + 1;
      end Increment;

      function Image (Value : Counter) return String is
      begin
         return Natural'Image (Natural (Value));
      end Image;
   end Counters;

   protected Gate is
      procedure Open;
      function Is_Open return Boolean;
   private
      Opened : Boolean := False;
   end Gate;

   protected body Gate is
      procedure Open is
      begin
         Opened := True;
      end Open;

      function Is_Open return Boolean is
      begin
         return Opened;
      end Is_Open;
   end Gate;

   task Worker is
      entry Start (At_Index : Item_Index);
   end Worker;

   task body Worker is
      First : Item_Index := Item_Index'First;
   begin
      accept Start (At_Index : Item_Index) do
         First := At_Index;
      end Start;
      Report (Label => "worker", Value => Scores (First));
   exception
      when Error : others =>
         Put_Line (Ada.Exceptions.Exception_Name (Error));
   end Worker;

   Local_Count : Counters.Counter;
   Cursor : Item_Index := Item_Index'First;
begin
   pragma Assert (Hex_Value = 51_966);
   Report ("astral", Clamp (Integer (Tiny_Value * Real_Value)), Loud => True);
   Put_Line (Unicode_Text);
   Put_Line ("quote: ""Ada"" and character " & Character'Image ('λ'));

   Worker.Start (At_Index => 2);
   Gate.Open;
   if Gate.Is_Open and then Current_Ref.Enabled then
      Counters.Increment (Local_Count);
   end if;

   Main_Loop : for Index in Scores'Range loop
      Scores (Index) := Clamp (Scores (Index) * Index);
      exit Main_Loop when Index = Scores'Last;
   end loop Main_Loop;

   while Cursor < Item_Index'Last loop
      Cursor := Item_Index'Succ (Cursor);
      Mask := Mask xor Permissions (Cursor);
   end loop;

   case Current.Id mod 4 is
      when 0 =>
         Report ("zero", 0);
      when 1 | 2 =>
         Report ("small", Current.Id);
      when others =>
         null;
   end case;

   declare
      Copy : Score_Array := Scores;
      Total : Integer := 0;
   begin
      for Value of Copy loop
         Total := Total + Value;
      end loop;
      Report ("total", Total);
   exception
      when Constraint_Error | Numeric_Error =>
         raise Problem with "aggregate failed";
   end;

   if Empty_Scores'Length = Scores'Length then
      Put_Line (Counters.Image (Local_Count));
   elsif Current.Enabled then
      delay 0.01;
   else
      raise Problem;
   end if;

exception
   when Problem =>
      Put_Line ("handled Problem");
   when Error : others =>
      Put_Line (Ada.Exceptions.Exception_Message (Error));
end Syntax_Atlas;
