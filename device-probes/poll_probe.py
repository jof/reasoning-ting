import ui, time
print("PING")
try:
    print("adc 0..7:", [ui.adc(i) for i in range(8)])
except Exception as e:
    print("adc(i) err:", e)
    try:
        print("adc():", ui.adc())
    except Exception as e2:
        print("adc() err:", e2)
prev = None
print("SQUEEZE NOW")
t0 = time.ticks_ms()
while time.ticks_diff(time.ticks_ms(), t0) < 6000:
    try:
        a = tuple(ui.adc(i) for i in range(8))
    except Exception:
        a = ()
    cur = (round(ui.handle(), 3), ui.handle_raw(), tuple(ui.sw(i) for i in range(5)), a)
    if cur != prev:
        print(time.ticks_diff(time.ticks_ms(), t0), cur)
        prev = cur
    time.sleep_ms(20)
print("END")
