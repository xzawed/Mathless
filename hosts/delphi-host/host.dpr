{ Mathless Delphi host — the OTHER half of D14, staged and waiting for a compiler.

  STATUS: NEVER COMPILED BY DELPHI. Free Pascal 3.2.2 in -Mdelphi mode built and ran it
  on 2026-09-07 and every check passed (GATE_DELPHI_OK), which is the first time any
  compiler had read this file -- and that run found a real defect in it, see GateOk below.
  It is NOT the Delphi verification: -Mdelphi is a dialect emulation, D14 names dcc64, and
  the dcc64 on this machine is an edition that refuses command-line builds (measured:
  dcc64, dcc32 and msbuild all print "does not support command line compiling", write
  nothing, and exit 0). So the UnicodeString-vs-PAnsiChar hazard the generated units warn
  about is still unmeasured -- it is an Embarcadero property that Free Pascal cannot show.
  Do not cite this file as Delphi evidence until MATHLESS_GATE_DELPHI=require has passed.

  WHAT IT IS MEANT TO PROVE, once it compiles and runs:
    - the GENERATED `.pas` units are valid Object Pascal and their declarations are
      correct, which is the half of D14 that `hosts/c-host` cannot speak to;
    - a Delphi host reads the same values across the same C ABI as the C host does;
    - the load-time gate (abi version + interface fingerprint) works from Delphi too.

  HOW IT DIFFERS FROM THE C HOST, ON PURPOSE. `hosts/c-host` resolves every symbol
  with LoadLibrary/GetProcAddress and can therefore DECLINE: it prints LOAD_FAIL and
  returns 2. The generated `.pas` instead declares `external ML_MODULE`, bound when
  the PROGRAM loads, so this host cannot decline to start -- the loader refuses first
  and the process never reaches `begin`. That is why the fingerprint check below is
  written as "refuse to USE" rather than "refuse to load".

  MEASURED (2026-09-07), after this paragraph had stood as an unchecked assertion:
  park one module and this host writes ZERO bytes and dies with 0xC0000135,
  STATUS_DLL_NOT_FOUND. It is pinned now by
  `the_staged_pascal_host_builds_and_calls_the_modules`, which renames a .dll away and
  requires the output to be empty. No amount of C-host coverage reaches this axis,
  because GetProcAddress binds nothing at load time.

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
  shapes;

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

{ The load-time gate, in the shape SPEC-iface-hash section 2.6 requires of a host.
  Unlike the C host this runs AFTER the loader has already bound the imports, so it
  gates USE rather than loading -- see the header note.

  EVERY name below is qualified, and that is not style. Each generated unit declares the
  same two reserved functions, so an unqualified `ml_iface_hash` binds to whichever unit
  came LAST in the uses clause. The first version of this file did exactly that: it
  compared carrier's fingerprint against ML_DISCOUNT_IFACE_HASH, which can never match.
  Measured the day this file was first compiled at all (2026-09-07):

      unqualified ml_iface_hash = C8D1191E339AAF6E   (carrier, the last unit used)
      ML_DISCOUNT_IFACE_HASH    = 05697A6FAFD68344

  `ml_module_abi_version` had the same bug and hid it, because every module answers 1.
  The C host cannot meet this at all: it resolves per module handle. It is a hazard of
  the import-unit binding, so the gate now covers every module used, not one. }
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
  if not OneModule('discount', discount.ml_module_abi_version, discount.ml_iface_hash,
                   ML_DISCOUNT_IFACE_HASH) then
    Result := False;
  if not OneModule('safe_div', safe_div.ml_module_abi_version, safe_div.ml_iface_hash,
                   ML_SAFE_DIV_IFACE_HASH) then
    Result := False;
  if not OneModule('carrier', carrier.ml_module_abi_version, carrier.ml_iface_hash,
                   ML_CARRIER_IFACE_HASH) then
    Result := False;
  if not OneModule('shapes', shapes.ml_module_abi_version, shapes.ml_iface_hash,
                   ML_SHAPES_IFACE_HASH) then
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

  { Q12: the caller owns the buffer. PAnsiChar, never UnicodeString — passing a
    UnicodeString here compiles, does not crash, and matches no code at all, which is
    the silent wrong answer HOST_ABI.md records and the module cannot detect. }
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
  Check(Status = 0, 'is_big(50) succeeds');
  Check(BigOut = False, 'a false bool out-param reads as false, not as three bytes of noise');
  ByteCanary := True;
  for I := 1 to 3 do
    if BoolBytes[I] <> $AA then
    begin
      ByteCanary := False;
      Break;
    end;
  Check(ByteCanary, 'the module wrote exactly one byte for a bool out-param');

  if Failures = 0 then
  begin
    Writeln('GATE_DELPHI_OK');
    Halt(0);
  end;
  Writeln('FAILURES: ', Failures);
  Halt(1);
end.
