test_stdout!(trailing_zeros_are_not_truncated, "<<\"12346\">>\n<<\"12345.7\">>\n<<\"12345.68\">>\n<<\"12345.679\">>\n<<\"12345.6789\">>\n<<\"12345.67890\">>\n<<\"12345.678900\">>\n<<\"12345.6789000\">>\n<<\"12345.67890000\">>\n<<\"12345.678900000\">>\n<<\"12345.6789000000\">>\n<<\"12345.67890000000\">>\n<<\"12345.678900000001\">>\n<<\"12345.6789000000008\">>\n<<\"12345.67890000000079\">>\n<<\"12345.678900000000795\">>\n<<\"12345.6789000000007945\">>\n");