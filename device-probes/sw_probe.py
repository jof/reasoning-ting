import ui, time
print("READY - squeeze repeatedly: first click, bottom, release")
prev = None
t0 = time.ticks_ms()
while time.ticks_diff(time.ticks_ms(), t0) < 8000:
    cur = tuple(ui.sw(i) for i in range(5))
    if cur != prev:
        print(time.ticks_diff(time.ticks_ms(), t0), "sw", cur, "hr", ui.handle_raw())
        prev = cur
    time.sleep_ms(10)
print("END")
