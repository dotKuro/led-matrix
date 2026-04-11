#!/usr/bin/env python3

import board
import neopixel
import time
import random

DATA_PIN_0 = board.D12
DATA_PIN_1 = board.D13

NUM_PIXELS = 512

def get_random_color():
    return (
        random.randint(30,150),
        random.randint(30,150),
        random.randint(30,150)
    )

if __name__ == "__main__":
    pixels_0 = neopixel.NeoPixel(
        DATA_PIN_0,
        NUM_PIXELS,
        pixel_order=neopixel.GRB,
        auto_write=False,
        brightness=0.05
    )

    pixels_1 = neopixel.NeoPixel(
        DATA_PIN_1,
        NUM_PIXELS,
        pixel_order=neopixel.GRB,
        auto_write=False,
        brightness=0.05
    )
    
    try:
        while True:
            for i in range(NUM_PIXELS):
                pixels_0[i] = get_random_color()
                pixels_1[i] = get_random_color()

            pixels_0.show()
            pixels_1.show()
            
            time.sleep(0.05)
    finally:
        pixels_0.fill((0,0,0))
        pixels_1.fill((0,0,0))

        pixels_0.show()
        pixels_1.show()
