import ui, time, array
B = array.array("I", [0] * 300); bi = 0
cnt = array.array("I", [0] * 16)          # per-ADC-channel event counts
def cb(m):
    global bi
    t = m >> 16                            # cached small int (<=31), no alloc
    if t == 1 or t == 2:                   # button press / release
        if bi < 300:
            B[bi] = m; bi += 1
    elif (t & 0xF0) == 0x10:               # ADC stream
        cnt[t & 0xF] += 1                  # array increment, no alloc
ui.callback(cb)
print("GO --- squeeze now, take your time")
time.sleep(12)
print("BUTTONS", bi)
for i in range(bi):
    print("BTN type", B[i] >> 16, "val", B[i] & 0xFFFF)
print("ADC activity:")
for a in range(16):
    if cnt[a]:
        print(" ch", a, "=", cnt[a])
print("END")
