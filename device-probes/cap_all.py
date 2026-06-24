import ui, time, array
E = array.array("I", [0] * 600); ei = 0
def cb(m):
    global ei
    if ei < 600:
        E[ei] = m; ei += 1          # raw capture, zero alloc, decode offline
ui.callback(cb)
print("GO --- squeeze slowly: first click, hold; bottom, hold; release. x2")
time.sleep(11)
print("N", ei)
for i in range(ei):
    m = E[i]
    print(m >> 16, m & 0xFFFF)      # type, value
print("END")
