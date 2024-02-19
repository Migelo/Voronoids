# Voronoids
A parallel code to compute Voronoi diagram for future cosmological surveys

## Todo

- [ ] Optimize identify non-conflict points in parallel version
- [ ] Optimize computing sphere volume and radius
- [ ] Optimize construction of neighbor relationship between the newly established points.
- [ ] Pipelining threads. Currently, there is a lot of dead time between pushing data into the new tree and computing. It would be nice to separate them using channel.