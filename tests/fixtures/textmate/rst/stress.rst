Atlas Builder Reference
=======================

.. _atlas-home:

Overview
--------

Atlas turns café records, λ-shaped routes, and observatory emoji 🔭 into reports.
This chapter exercises **strong text**, *emphasis*, ``monospaced values``, and an
escaped \*asterisk\*.  Visit the `Atlas portal <https://example.org/atlas>`_,
follow portal_, or jump to :ref:`the pipeline <pipeline>`.

.. _portal: https://example.org/portal

Metadata and options
--------------------

:Author: Ada Example
:Revision: 0x2A
:Status: *reviewed*
:Unicode: Ελληνικά, 東京, and 🧭

-v  enable concise diagnostics
--verbose  enable detailed diagnostics
--format=json  choose a machine-readable format
-o <FILE>, --output <FILE>  write the report to FILE
/Q  request quiet operation on compatible systems

.. note:: Keep source coordinates intact.
   :class: important atlas-note
   :name: coordinate-note

   The renderer preserves :math:`λ + π` and :code:`point = (3, 5)`.
   It also recognizes |compass| inside directive content.

.. warning:: Validate imported layers before publishing.
   :class: caution

   A malformed edge can disconnect the entire route graph.

.. |compass| replace:: 🧭
.. |product| replace:: **Atlas Builder**
.. |nbsp| unicode:: 0xA0

.. include:: shared-introduction.rst
   :start-after: overview-start
   :end-before: overview-end

Lists and lines
---------------

* Load a source map.
* Normalize each café name.
* Render the selected layers.

1. Inspect the preview.
2. Compare the checksum.
3. Publish only after approval.

a. Keep a local archive.
b. Record the release label.

| North: snow fields ❄
| East: dawn routes
| South: warm harbors
| West: observatory 🌌

Pipeline
--------

.. _pipeline:

The pipeline has three stages [Design]_ and one optional audit [#audit]_.
Automatic notes [#]_ can coexist with symbolic notes [*]_.

.. [Design] “Layered Cartography”, Example Press, 2026.
.. [#audit] The audit stores no personal coordinates.
.. [#] This automatically numbered note explains normalization.
.. [*] The symbol note marks experimental behavior.

.. py:function:: build_atlas(source, *, projection="mercator")
   :module: atlas.builder
   :param source: Path to a *validated* source.
   :param projection: Projection name; default is ``mercator``.
   :returns: A rendered :class:`Atlas` instance.

   Build a map and retain links to source features.

.. cpp:function:: render
   :noindex:

   The native renderer is used for very large layers.

.. js:function:: preview(options)
   :async:

   Returns a browser preview for the requested options.

.. autofunction:: atlas.audit.verify
   :noindex: 1

   This generated entry documents the verification hook.

Code examples
-------------

.. code-block:: python
   :linenos:
   :caption: Build a tiny atlas
   :emphasize-lines: 2

   from atlas import Builder
   builder = Builder("Málaga")
   print(builder.render(symbol="🚀"))

.. code-block:: javascript
   :caption: Preview in a browser

   const title = "東京 layer";
   console.log(`${title} 🗺️`);

.. code-block:: cpp
   :caption: Native projection

   auto name = std::string{"naïve"};
   render(name, 0x2a);

.. code-block:: yaml

   atlas: "Δelta"
   layers:
     - roads
     - "stars 🌟"

.. code-block:: console

   $ atlas build --format=json
   rendered 12 layers

.. code-block:: cmake

   project(Atlas LANGUAGES CXX)
   add_executable(atlas main.cpp)

.. code-block:: ruby

   puts "route №7 🚲"

.. code-block:: Kconfig

   config ATLAS_PREVIEW
       bool "Enable preview"

.. code-block:: dts

   atlas@0 {
       compatible = "example,atlas";
   };

.. code-block:: text

   Plain fallback content remains indented.
   Its second line closes before the next heading.

Doctest and literal material
----------------------------

>>> sum([2, 3, 5])
10
>>> "café".upper()
'CAFÉ'

.. doctest::

   >>> marker = "🧪"
   >>> len(marker)
   1

A trailing double colon introduces a literal block::

   SELECT name, latitude
   FROM places
   WHERE label = 'São Paulo';

The paragraph after the indentation proves that the literal state is closed.

Tables
------

+-----------+------------+----------+
| Layer     | Projection | Status   |
+===========+============+==========+
| streets   | Mercator   | ready    |
+-----------+------------+----------+
| galaxies  | Aitoff     | draft 🌠 |
+-----------+------------+----------+

=======  =========  ======
Name     Kind       Count
=======  =========  ======
roads    vector     12
labels   text       81
=======  =========  ======

Links, roles, and references
----------------------------

Use :ref:`pipeline`, :doc:`installation`, and :download:`sample <atlas.zip>`.
The named reference portal_ differs from the anonymous reference `mirror`__.

__ https://mirror.example.org/atlas

The phrase `Atlas Builder`_ names a conventional target, while |product|
expands a substitution.  ``literal *stars*`` remain literal, but **these
stars are bold** and *these words are emphasized*.

.. _Atlas Builder: https://example.org/atlas-builder

Comments and raw output
-----------------------

.. This comment describes an implementation detail.
   It spans multiple indented lines and mentions Ω and 🐉.
   None of it belongs to the rendered manual.

Visible prose resumes here, closing the comment block.

.. raw:: html
   :class: atlas-fragment

   <aside lang="el">Χάρτης <strong>Atlas</strong> 🌍</aside>

.. image:: compass.png
   :alt: Compass rose 🧭
   :width: 320
   :target: https://example.org/compass

Final checks
------------

The final paragraph cites [Design]_, resolves |compass|, and ends every block.
