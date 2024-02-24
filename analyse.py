import collections
import matplotlib.pyplot as plt
import numpy as np
import json

with open('target/sections.json') as fp:
    data = json.load(fp)

def bin_color(color_node):
    sumlist = []
    for _name, section in data.items():
        c = collections.Counter(section[color_node])
        v = [e for _, e in c.most_common()]
        sumlist[0:0] = v
    return sumlist

for color_node in ['nodes_gray', 'nodes_red', 'nodes_green', 'nodes_blue']:
    sumlist = bin_color(color_node)
    if not sum(sumlist):
        continue

    plt.hist(sumlist)
    plt.show()
