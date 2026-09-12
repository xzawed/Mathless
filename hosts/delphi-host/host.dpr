{ Mathless Delphi host — the OTHER half of D14. Gated locally, not in CI.

  STATUS: GATED ON A DEVELOPER MACHINE. MATHLESS_GATE_DELPHI builds this file with dcc64
  and runs it, and it passes. The dcc64 on this machine is an edition that refuses
  command-line builds (measured: dcc64, dcc32 and msbuild all print "does not support
  command line compiling", write nothing, and exit 0), so the gate drives bds.exe -b
  instead, which the same edition DOES allow.

  The gate does not run in CI: the runner has neither Delphi nor an interactive desktop
  session. So CI cannot catch a break here -- whoever changes the generator or this host
  has to run the gate themselves.

  History, because this header has twice said something that had stopped being true:
  Free Pascal 3.2.2 in -Mdelphi mode built and ran this file first, on 2026-09-07
  (GATE_DELPHI_OK) -- the first time any compiler had read it, and that run found a real
  defect in it, see GateOk below. FPC is NOT the Delphi verification: -Mdelphi is a dialect
  emulation and D14 names dcc64. Delphi itself followed on 2026-09-07 by hand (9-15), and
  the gate that repeats it arrived on 2026-09-09 (9-20).

  The UnicodeString-vs-PAnsiChar hazard is no longer unmeasured: this file writes all three
  spellings on purpose and pins what each one sends (see the section near the end). That
  measurement corrected HOST_ABI.md rather than confirming it. It also showed the hazard is
  NOT purely an Embarcadero property -- Free Pascal sends the same bytes for an explicit
  UnicodeString. What Free Pascal cannot show is the part that actually bites: in Delphi
  plain `string` IS UnicodeString, and in -Mdelphi it is AnsiString, so the same source
  line is wrong here and right there.

  WHAT IT IS MEANT TO PROVE, once it compiles and runs:
    - the GENERATED `.pas` units are valid Object Pascal and their declarations are
      correct, which is the half of D14 that `hosts/c-host` cannot speak to;
    - a Delphi host reads the same values across the same C ABI as the C host does;
    - the load-time gate (abi version + interface fingerprint) works from Delphi too.

  HOW IT DIFFERS FROM THE C HOST, ON PURPOSE. `hosts/c-host` resolves every symbol
  with LoadLibrary/GetProcAddress, so a missing module is data it can report: it prints
  `FAIL LoadLibraryA(<path>) -> error 126`, counts a failure and CARRIES ON. The
  generated `.pas` instead declares `external ML_MODULE`, bound when the PROGRAM loads,
  so this host cannot report anything -- the loader refuses first and the process never
  reaches `begin`. That is why the fingerprint check below is written as "refuse to USE"
  rather than "refuse to load".

  BOTH SIDES MEASURED (2026-09-07), after this paragraph had stood unchecked -- and the
  first version of it described the C host from memory and got it wrong. Park carrier.dll
  and run each:

    C host        78 lines of checks completed first, then FAIL LoadLibraryA ... 126, exit 1
    this host     0 bytes of output, exit 0xC0000135 (STATUS_DLL_NOT_FOUND)

  Pinned by `the_staged_pascal_host_builds_and_calls_the_modules`, which parks a .dll and
  requires the output to be EMPTY. No amount of C-host coverage reaches this axis, because
  GetProcAddress binds nothing at load time.

  BUILD (once dcc64 exists), from a directory holding the generated artifacts:
      dcc64 -U<artifact_dir> host.dpr
      host.exe <expected_abi_version>
}
program host;

{$APPTYPE CONSOLE}

uses
  SysUtils,
  discount,
  safe_div,
  carrier,
  shapes,
  basket,
  { SPEC-array-return acceptance E. `in_stock` returns `[bool]`, and the element WIDTH is
    the hazard that slice added: the module writes ONE byte per element, and a host that
    holds `array of LongBool` reads a stride that does not exist. Measured below, not
    warned about. }
  allocate;

var
  Failures: Integer = 0;

procedure Check(Ok: Boolean; const What: string);
begin
  if Ok then
    Writeln('  ok   ', What)
  else
  begin
    Writeln('  FAIL ', What);
    Inc(Failures);
  end;
end;

{ What bytes are actually at this pointer? The three-spelling measurement turns on this:
  the status says a spelling is wrong, and this says why. Callers pass a count that is
  known to be in bounds -- an AnsiString's Length+1, a UnicodeString's Length*2+2. }
function HexAt(P: Pointer; N: Integer): string;
var
  I: Integer;
  B: PByte;
begin
  Result := '';
  B := PByte(P);
  for I := 1 to N do
  begin
    Result := Result + IntToHex(B^, 2) + ' ';
    Inc(B);
  end;
end;

{ The load-time gate, in the shape SPEC-iface-hash section 2.6 requires of a host.
  Unlike the C host this runs AFTER the loader has already bound the imports, so it
  gates USE rather than loading -- see the header note.

  EVERY name below is qualified, and that is not style. Each generated unit declares the
  same reserved `ml_module_abi_version`, so an unqualified use binds to whichever unit came
  LAST in the uses clause. The first version of this file did exactly that: it compared
  carrier's fingerprint against ML_DISCOUNT_IFACE_HASH, which can never match. Measured the
  day this file was first compiled at all (2026-09-07):

      unqualified ml_iface_hash = C8D1191E339AAF6E   (carrier, the last unit used)
      ML_DISCOUNT_IFACE_HASH    = 05697A6FAFD68344

  `ml_module_abi_version` had the same bug and hid it, because every module answers 1.

  The FINGERPRINT half of that trap is gone at the source since SPEC-qualified-iface-hash:
  the export is `ml_iface_hash_<module>`, so the four calls below name four different
  functions and could not bind to one another even unqualified. The unit prefixes stay,
  because `ml_module_abi_version` still can -- and because the rule "qualify every imported
  name" is one rule instead of a rule with an exception in it.

  The C host never met the fingerprint half at all: it resolves per module handle. The
  LINKED C host did meet it, silently, which is what the rename fixes -- so this hazard was
  never Pascal's alone, only found there first. }
function GateOk(ExpectedAbi: LongWord): Boolean;

  function OneModule(const Name: string; Abi: LongWord; Hash, Pinned: UInt64): Boolean;
  begin
    Result := True;
    if Abi <> ExpectedAbi then
    begin
      Writeln('  refuse: ', Name, ' abi ', Abi, ', host built for ', ExpectedAbi);
      Result := False;
    end;
    if Hash <> Pinned then
    begin
      Writeln('  refuse: ', Name, ' fingerprint differs from the one its unit pins');
      Result := False;
    end;
  end;

begin
  Result := True;
  if not OneModule('discount', discount.ml_module_abi_version,
                   discount.ml_iface_hash_discount, ML_DISCOUNT_IFACE_HASH) then
    Result := False;
  if not OneModule('safe_div', safe_div.ml_module_abi_version,
                   safe_div.ml_iface_hash_safe_div, ML_SAFE_DIV_IFACE_HASH) then
    Result := False;
  if not OneModule('carrier', carrier.ml_module_abi_version,
                   carrier.ml_iface_hash_carrier, ML_CARRIER_IFACE_HASH) then
    Result := False;
  if not OneModule('shapes', shapes.ml_module_abi_version,
                   shapes.ml_iface_hash_shapes, ML_SHAPES_IFACE_HASH) then
    Result := False;
  { `basket` was USED and not gated, for the whole of two slices, while the paragraph above
    said "the gate now covers every module used, not one". A guard's SCOPE is a claim too,
    and that one had quietly stopped being true the day the unit was added. Both of the
    stragglers are here now, and `the_delphi_host_gates_every_unit_it_uses` derives the list
    from the `uses` clause so the next unit cannot be forgotten the same way. }
  if not OneModule('basket', basket.ml_module_abi_version,
                   basket.ml_iface_hash_basket, ML_BASKET_IFACE_HASH) then
    Result := False;
  if not OneModule('allocate', allocate.ml_module_abi_version,
                   allocate.ml_iface_hash_allocate, ML_ALLOCATE_IFACE_HASH) then
    Result := False;
end;

var
  ExpectedAbi: LongWord;
  Status: Integer;
  OutValue: Double;
  Untouched: Double;
  Buf: array[0..63] of Byte;
  Needed: Integer;
  Tier: Integer;
  I: Integer;
  Canary: Boolean;
  { A Boolean out-param that shares storage with four bytes we can inspect. `absolute` is
    the Pascal way to ask what the module actually wrote. }
  BoolBytes: array[0..3] of Byte;
  BigOut: Boolean absolute BoolBytes;
  ByteCanary: Boolean;
  { The three spellings of "UnicodeString -> PAnsiChar" that HOST_ABI.md rule 5 separates. }
  S: UnicodeString;
  A: AnsiString;
  T: string;
  { Array input. DYNAMIC arrays on purpose: that is what a Delphi host actually holds, and
    the empty one is where its natural idiom breaks (SPEC-array-input 5.2). }
  Qty, Price: array of Integer;
  Empty: array of Integer;
  Xs: array of Double;
  Flags: array of Boolean;
  IntOut, Tally: Integer;
  DblOut: Double;
  BoolOut: Boolean;
  ElemsP: PInteger;
  { Array RETURN of `[bool]` (SPEC-array-return acceptance E). `Stock` is the input;
    `Flags1` is the correct 1-byte reading; `Flags4` is the same bytes read through
    four-byte booleans, which is the mistake being measured. `Flags4Bytes` is the storage
    they share, so the transcript can print what the module actually wrote. }
  Stock: array of Integer;
  Flags1: array of Boolean;
  Flags4: array of LongBool;
  Wrote: string;
begin
  if ParamCount < 1 then
  begin
    Writeln('usage: host <expected_abi_version>');
    Halt(2);
  end;
  ExpectedAbi := StrToInt(ParamStr(1));

  if not GateOk(ExpectedAbi) then
  begin
    Writeln('GATE_DELPHI_REFUSED');
    Halt(1);
  end;
  Check(True, 'discount.dll passed the abi + interface gate');

  { Scalars. `vip` is a 1-byte Boolean on both sides — the generated unit says so, and
    using LongBool here would read three bytes of noise. }
  Check(mlx_discount(100.0, True) = 90.0, 'mlx_discount(100, true) = 90');
  Check(mlx_discount(100.0, False) = 100.0, 'mlx_discount(100, false) = 100');

  { D17: status plus an out-param, and the out-param is NOT written when the call fails. }
  OutValue := -1.0;
  Status := mlx_safe_div(10.0, 2.0, OutValue);
  Check(Status = 0, 'safe_div(10, 2) status = 0');
  Check(OutValue = 5.0, 'safe_div(10, 2) writes 5 through the out-param');

  Untouched := 12345.0;
  Status := mlx_safe_div(1.0, 0.0, Untouched);
  Check(Status = ML_SAFE_DIV_ERR_DIV_BY_ZERO, 'safe_div(1, 0) status = ML_SAFE_DIV_ERR_DIV_BY_ZERO');
  Check(Untouched = 12345.0, 'a failed call leaves the out-param untouched');

  { Q12: the caller owns the buffer. PAnsiChar, never UnicodeString -- measured at the
    end of this file: a UnicodeString compiles, does not crash, and delivers its first
    UTF-16 unit as a one-character string, so the module answers an ordinary "unknown
    code" and nothing downstream can tell that from a real one. }
  FillChar(Buf, SizeOf(Buf), $AA);
  Needed := -1;
  Status := mlx_carrier_name(PAnsiChar('UPSN'), @Buf[0], SizeOf(Buf), Needed);
  Check(Status = 0, 'carrier_name(UPSN) succeeds');
  Check(PAnsiChar(@Buf[0]) = 'UPS Ground', 'carrier_name(UPSN) = "UPS Ground"');

  { Truncation is a FAILURE and writes nothing. One byte short of "UPS Ground" + NUL. }
  FillChar(Buf, SizeOf(Buf), $AA);
  Needed := -1;
  Status := mlx_carrier_name(PAnsiChar('UPSN'), @Buf[0], 10, Needed);
  Check(Status < 0, 'one byte short is a failure, not a short success');
  Check(Needed = 11, 'needed is exact on the failure path');
  Canary := True;
  for I := 0 to SizeOf(Buf) - 1 do
    if Buf[I] <> $AA then
    begin
      Canary := False;
      Break;
    end;
  Check(Canary, 'not one byte of the buffer was written');

  { A declared out plus the buffer triple, in DP-O1 order: declared outs first. }
  FillChar(Buf, SizeOf(Buf), 0);
  Tier := -1;
  Needed := -1;
  Status := mlx_carrier_label(PAnsiChar('UPSN'), Tier, @Buf[0], SizeOf(Buf), Needed);
  Check(Status = 0, 'carrier_label(UPSN) succeeds');
  Check(Tier = 1, 'the declared out comes before the buffer triple');

  { The unit's header says "Boolean is 1 byte to match the module ABI; do not use LongBool".
    Nothing checked it, and it is not decoration: measured 2026-09-07 by declaring LongBool
    instead, the module still writes exactly one byte, Pascal reads four, and a FALSE comes
    back as TRUE. No crash. The module said false and the host believed true -- the silent
    wrong answer this project exists to prevent.

    Two checks, because they catch different things. The VALUE catches a wrong declaration
    (LongBool passes the canary and fails here). The CANARY catches a module writing more
    than a byte (which LongBool would not reveal). Neither alone is enough.

    Only a Pascal host can ask this. The C header says `bool`, which C sizes for itself. }
  FillChar(BoolBytes, SizeOf(BoolBytes), $AA);
  Status := mlx_is_big(50, BigOut);
  Check(Status = 0, 'is_big(50) succeeds (precondition: on failure the out is untouched)');
  Check(BigOut = False, 'a false bool out-param reads as false, not as three bytes of noise');
  ByteCanary := True;
  for I := 1 to 3 do
    if BoolBytes[I] <> $AA then
    begin
      ByteCanary := False;
      Break;
    end;
  Check(ByteCanary, 'the module wrote exactly one byte for a bool out-param');

  { HOST_ABI.md's string rule 5 names THREE spellings of "UnicodeString -> PAnsiChar" and
    says which are right. Until this ran they were all E1 -- Embarcadero documentation,
    never measured, because no Delphi had ever compiled this file. The generated unit
    carries the same three lines, and the comment above carrier_name in this file asserted
    the hazard as fact with nothing checking it.

    `carrier_name` is the instrument. It answers 0 + "UPS Ground" for exactly the four
    bytes U P S N and E_UNKNOWN_SCAC (1) for anything else, so the status alone separates
    the spellings; the byte dumps say why. `S` is ASCII on purpose -- rule 4 says the
    module compares bytes, and the source literal is ASCII-only. }
  S := 'UPSN';

  { 1. The documented-correct spelling: an AnsiString in a LOCAL, so it outlives the call. }
  A := AnsiString(S);
  FillChar(Buf, SizeOf(Buf), 0);
  Needed := -1;
  Status := mlx_carrier_name(PAnsiChar(A), @Buf[0], SizeOf(Buf), Needed);
  Check((Status = 0) and (PAnsiChar(@Buf[0]) = 'UPS Ground'),
    'PAnsiChar(AnsiString(S)) from a local: status ' + IntToStr(Status) + ', sent ' +
    HexAt(PAnsiChar(A), Length(A) + 1));

  { 2. Directly on the UnicodeString. HOST_ABI.md said this warns and CONVERTS through the
       ANSI code page, so ASCII survives. It does not. Measured: the same UTF-16 bytes as
       spelling 3, byte for byte -- it is a hard pointer cast, not a conversion. What is
       true is that the compiler objects: dcc64 answers `W1044 Suspicious typecast of
       string to PAnsiChar` (read out of the IDE build's host.err, 2026-09-10). So this
       spelling is wrong but NOT silent, which is the opposite of what the doc implied. }
  FillChar(Buf, SizeOf(Buf), 0);
  Needed := -1;
  Status := mlx_carrier_name(PAnsiChar(S), @Buf[0], SizeOf(Buf), Needed);  { ML_W1044 }
  Check(Status = 1,
    'PAnsiChar(S) does NOT convert -- it reinterprets, exactly like Pointer(S): status ' +
    IntToStr(Status) + ', sent ' + HexAt(PAnsiChar(S), Length(S) * 2 + 2));  { ML_W1044 }

  { 3. The silent one. Documented: reinterprets the UTF-16 bytes, so the module reads to
       the first NUL and sees ONE character. No crash, and an ordinary "unknown code"
       failure -- which is why nothing downstream can tell it from a real one. }
  FillChar(Buf, SizeOf(Buf), 0);
  Needed := -1;
  Status := mlx_carrier_name(PAnsiChar(Pointer(S)), @Buf[0], SizeOf(Buf), Needed);  { ML_NO_DIAGNOSTIC }
  Check(Status = 1,
    'PAnsiChar(Pointer(S)) fails as an unknown code, not a crash: status ' +
    IntToStr(Status) + ', sent ' + HexAt(Pointer(S), Length(S) * 2 + 2));

  { 4. And this is where it actually bites, because it is what a host would naturally
       write. Delphi's `string` IS UnicodeString, so `PAnsiChar(T)` here is spelling 2 --
       wrong, though W1044 says so. Free Pascal in -Mdelphi mode does NOT emulate that:
       its `string` is AnsiString, so the identical source is CORRECT there.

       The same four lines, two compilers, two different answers. That is the sharpest
       measurement in this file of why `-Mdelphi` is a dialect emulation and not Delphi,
       and why MATHLESS_GATE_FPC_HOST can never stand in for MATHLESS_GATE_DELPHI. }
  { 5. The correct spelling again, but INLINE instead of through a local. The advice this
       project has repeated since the string slice -- "keep the AnsiString in a local for the
       duration of the call" -- reads as if the call itself needs it. Measured here, because
       it was carried for a month without being measured: the temporary a cast expression
       makes lives to the end of the STATEMENT, and the call IS that statement, so one call
       is fine. What the advice is really about is not STORING the pointer for later, and
       that failure is undefined behaviour -- a run where it appears to work proves nothing,
       so this file does not pretend to measure it. }
  FillChar(Buf, SizeOf(Buf), 0);
  Needed := -1;
  Status := mlx_carrier_name(PAnsiChar(AnsiString(S)), @Buf[0], SizeOf(Buf), Needed);
  Check((Status = 0) and (PAnsiChar(@Buf[0]) = 'UPS Ground'),
    'PAnsiChar(AnsiString(S)) INLINE is fine for the duration of one call: status ' +
    IntToStr(Status));

  T := 'UPSN';
  FillChar(Buf, SizeOf(Buf), 0);
  Needed := -1;
  Status := mlx_carrier_name(PAnsiChar(T), @Buf[0], SizeOf(Buf), Needed);  { ML_W1044 }
{$IFDEF FPC}
  Check(Status = 0,
    'Free Pascal: `string` is AnsiString, so PAnsiChar(T) is the CORRECT spelling here -- ' +
    'status ' + IntToStr(Status) + ' (Delphi answers 1 for this same line)');
{$ELSE}
  Check(Status = 1,
    'Delphi: `string` IS UnicodeString, so the natural PAnsiChar(T) sends UTF-16 -- ' +
    'status ' + IntToStr(Status) + ' (Free Pascal answers 0 for this same line)');
{$ENDIF}

  { ---- Array INPUT (SPEC-array-input). ----

    A Pascal host is the one that can ask two things the C host cannot. First, the generated
    unit declares `xs: PInteger; xs_len: Integer` -- TWO parameters for one surface argument --
    and a host that gets them out of order does not compile. Second, `Boolean` here is one
    byte, which is the size the module reads per element. }
  SetLength(Qty, 3);
  Qty[0] := 2;  Qty[1] := 1;  Qty[2] := 4;
  SetLength(Price, 3);
  Price[0] := 30;  Price[1] := 500;  Price[2] := 25;

  IntOut := -1;
  Status := mlx_basket_total(@Qty[0], Length(Qty), @Price[0], Length(Price), 100, IntOut);
  Check((Status = 0) and (IntOut = 260), 'basket_total reads three lines in ONE call: ' +
    IntToStr(IntOut));

  Tally := 12345;
  Status := mlx_basket_total(@Qty[0], Length(Qty), @Price[0], 2, 100, Tally);
  Check(Status = ML_BASKET_ERR_E_LENGTH_MISMATCH,
    'mismatched lengths are the module''s domain error');
  Check(Tally = 12345, 'and a failed call writes no out-param');

  { The reserved negative. Only a host can reach it -- every loop inside the module is
    bounded by `len`. }
  Tally := 12345;
  Status := mlx_pick(@Qty[0], Length(Qty), 3, Tally);
  Check(Status = ML_ST_INDEX_OUT_OF_RANGE,
    'an out-of-range index is ML_ST_INDEX_OUT_OF_RANGE, a constant the UNIT declares');
  Check(Tally = 12345, 'and it leaves the out-param untouched');

  { ---- The hazard this slice creates, measured rather than warned about. ----

    `@arr[0]` is how a Delphi host naturally takes the address of a dynamic array. On an
    EMPTY one there is no element 0. With range checking off -- the default in a release
    build, and what this host compiles under -- it does not raise: it yields the array's
    own (nil) pointer, which is exactly what the module wants for a length of 0, because it
    never dereferences. That is why this is a trap and not a crash: it works, until someone
    builds with the $R+ directive and it starts raising instead.

    The idiom the generated unit recommends is the one below: ask Length first. }
  SetLength(Empty, 0);
  if Length(Empty) = 0 then
    ElemsP := nil
  else
    ElemsP := @Empty[0];
  Check(ElemsP = nil, 'the safe idiom yields nil for an empty array, and never indexes it');

  SetLength(Xs, 3);
  Xs[0] := 1.5;  Xs[1] := 9.25;  Xs[2] := -3.0;
  DblOut := 0.0;
  Status := mlx_largest(@Xs[0], Length(Xs), 0.0, DblOut);
  Check((Status = 0) and (DblOut = 9.25), 'largest over an f64 array');

  DblOut := -1.0;
  Status := mlx_largest(PDouble(nil), 0, 42.0, DblOut);
  Check((Status = 0) and (DblOut = 42.0),
    'an EMPTY array with a nil pointer is an ordinary answer: the module never dereferences');

  { One byte per element, on both sides. Only the last flag is set, so a module reading four
    bytes per Boolean would walk past the end and answer from whatever follows. }
  SetLength(Flags, 4);
  Flags[0] := False;  Flags[1] := False;  Flags[2] := False;  Flags[3] := True;
  Check(SizeOf(Boolean) = 1, 'Pascal Boolean is one byte -- the module ABI''s element size');
  Check(mlx_how_many(@Flags[0], Length(Flags)) = 4, 'len is the number the host passed');
  BoolOut := False;
  Status := mlx_any_set(@Flags[0], Length(Flags), BoolOut);
  Check((Status = 0) and BoolOut, 'the fourth Boolean is set, and it is one byte along');

  { ---- Array RETURN, and the element WIDTH (SPEC-array-return acceptance E). ----

    Array input proved the module READS one byte per Boolean. A return is the other
    direction: the module WRITES one byte per element into the host's buffer, and the host
    decides what stride to read it back with. Only a Pascal host can get that wrong in an
    interesting way -- the C header says `bool`, which C sizes for itself, while Pascal
    offers both a 1-byte `Boolean` and a 4-byte `LongBool` and lets you point either at the
    same memory. The generated unit says "do not use LongBool"; this is the measurement
    behind that sentence.

    Probe first, because ml_cap/ml_needed count ELEMENTS here and the retry is SetLength,
    never a byte count. }
  SetLength(Stock, 4);
  Stock[0] := 5;  Stock[1] := 2;  Stock[2] := 0;  Stock[3] := 7;

  Needed := -1;
  Status := mlx_in_stock(@Stock[0], Length(Stock), PBoolean(nil), 0, Needed);
  Check(Status = allocate.ML_ST_INSUFFICIENT_BUFFER,
    'a cap of 0 truncates, and the status is a constant the UNIT declares -- ' +
    'it did not declare it for an array-only module until this gate was written');
  Check(Needed = 4, 'and ml_needed is the length in ELEMENTS: ' + IntToStr(Needed));

  { The correct reading: one byte per element, which is what SizeOf(Boolean) already is. }
  SetLength(Flags1, Needed);
  Needed := -1;
  Status := mlx_in_stock(@Stock[0], Length(Stock), @Flags1[0], Length(Flags1), Needed);
  Check((Status = 0) and (Needed = 4), 'in_stock answers for four warehouses in ONE call');
  Check(Flags1[0] and Flags1[1] and (not Flags1[2]) and Flags1[3],
    'stock 5,2,0,7 -> in stock TRUE,TRUE,FALSE,TRUE, read as 1-byte Booleans');

  { Now step on it DELIBERATELY. `Flags4` is `array of LongBool`, four bytes per element.
    The module still writes FOUR BYTES IN TOTAL -- one per element, ml_cap elements -- so
    nothing is overrun and nothing raises: the host simply reads them at the wrong stride.

    The array is zero-filled by SetLength, so what the wrong reading returns is determined,
    not whatever happened to be on the heap. Element 0 covers the module's four written
    bytes 01 01 00 01, which is a NONZERO LongBool, i.e. True; elements 1..3 cover bytes the
    module never touched, i.e. False. So the host that gets the width wrong is told
    TRUE,FALSE,FALSE,FALSE where the module said TRUE,TRUE,FALSE,TRUE.

    No crash, no status, no warning -- a wrong answer. That is the whole point of measuring
    it instead of writing "do not use LongBool" and hoping. }
  SetLength(Flags4, Needed);
  Needed := -1;
  Status := mlx_in_stock(@Stock[0], Length(Stock), PBoolean(@Flags4[0]), Length(Flags4),
                         Needed);
  Check((Status = 0) and (Needed = 4),
    'the wrong-width call SUCCEEDS -- the module cannot see the host''s stride');

  Wrote := '';
  for I := 0 to Length(Flags4) - 1 do
    if Flags4[I] then Wrote := Wrote + 'T' else Wrote := Wrote + 'F';
  Writeln('  MEASURED [bool] return read as LongBool: ', Wrote,
          '   (1-byte Boolean reads TTFT)');
  Check(Wrote = 'TFFF',
    'LongBool reads TFFF where the module wrote TTFT: the first element swallows all four ' +
    'bytes and the rest read storage the module never wrote. Recorded, not warned about');
  Check(SizeOf(LongBool) = 4, 'control: LongBool really is four bytes here');

  if Failures = 0 then
  begin
    Writeln('GATE_DELPHI_OK');
    Halt(0);
  end;
  Writeln('FAILURES: ', Failures);
  Halt(1);
end.
