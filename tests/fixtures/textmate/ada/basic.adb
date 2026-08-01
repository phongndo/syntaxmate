with Ada.Text_IO;
use Ada.Text_IO;

procedure Basic is
   -- Unicode: café λ 東京 🚀 𝌆
   type Mood is (Quiet, Curious, Ready);
   subtype Small_Count is Natural range 0 .. 10;
   Count   : Small_Count := 3;
   Message : constant String := "café λ 東京 🚀 𝌆";

   function Doubled (Value : Integer) return Integer is
   begin
      return Value * 2;
   end Doubled;
begin
   for Index in 1 .. Count loop
      Put_Line (Message & Integer'Image (Doubled (Index)));
   end loop;

   if Count > 0 then
      null;
   else
      raise Program_Error with "unexpected empty range";
   end if;
end Basic;
